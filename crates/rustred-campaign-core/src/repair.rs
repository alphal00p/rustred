//! Live repair as a library (plan 3.9, R22; interface `interfaces/repair-library.md`): the traits through which the
//! engine (lane C-LIVE) or the campaign (lane I-INT) plug in the case solver, the per-key facts, the key prover and
//! the tables in force; a sequential in-process driver of the ladder ([`drive_tree`]); and [`repair_key`].
//! No clock, no files, no global state: budgets are arguments; CPU is what the solver reports.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::cover::{Dispatch, Dispatcher};
use crate::order::NodeComparator;
use crate::worklist::items::{CaseSpec, EvidenceLevel, Item, ItemId, Key, NodeParams, repair_item};
use crate::worklist::ladder::{
    Attempt, CandidateRule, Disposition, ItemState, KeyEvidence, LadderExhausted, Next, PointFacts,
    SolveAnswer, SolveOutcome, Tri, judge_points_with_hits, next_steps,
};
use crate::worklist::seed::{AttemptBudget, SeedSetting};
use crate::worklist::supplied::CandidateSource;

/// One engine call for ONE case under ONE seed setting (C-LIVE: the node driver in process, serialised).
pub trait CaseSolver {
    fn solve(&mut self, req: &CaseRequest, budget: &AttemptBudget) -> SolveAnswer;
}

/// What the solver is asked to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseRequest {
    pub node: String,
    pub case: CaseSpec,
    pub setting: SeedSetting,
    pub rung: crate::worklist::ladder::Rung,
    /// `engine_seed_args(setting)`: the ONE mapping to argv (EC-1 spelling).
    pub seed_args: Vec<String>,
    pub attempt_key: String,
}

impl CaseRequest {
    pub fn from_attempt(node: &str, a: &Attempt) -> Self {
        CaseRequest {
            node: node.to_string(),
            case: a.case.clone(),
            setting: a.setting.clone(),
            rung: a.rung,
            seed_args: a.seed_args.clone(),
            attempt_key: a.key.clone(),
        }
    }
}

/// Per-key facts of a candidate (rr_proof instantiate as a library, EC-2). Regularity is decided PER KEY by exact
/// evaluation of the denominators / witness loci at the key (R38), never by factoring.
pub trait FactSource {
    fn facts(&mut self, rule: &CandidateRule, key: &[i32]) -> PointFacts;
}

/// Descent under the FULL node comparator (with its tie-breaks): `lower(a, b)` = a is lower than b.
pub trait Descent {
    fn lower(&self, a: &[i32], b: &[i32]) -> bool;
}

impl Descent for NodeComparator {
    /// Checked comparison (re-audit RB m5): a key of the wrong length is "not lower" (fail closed), never a panic.
    fn lower(&self, a: &[i32], b: &[i32]) -> bool {
        NodeComparator::try_lower(self, a, b).unwrap_or(false)
    }
}

/// Identity evidence at keys, at the level the node's spec requires (U-12: `replay` = A2, `per-key` = A3).
pub trait KeyProver {
    fn prove(
        &mut self,
        rule: &CandidateRule,
        keys: &[Key],
        wanted: EvidenceLevel,
    ) -> Vec<KeyEvidence>;
}

/// Optimise items (U-13): does the candidate's domain contain the old rules' domains (R33: exact partition proof on
/// FULL nodes)?
pub trait ContainmentCheck {
    fn contains(&mut self, rule: &CandidateRule, old: &[String]) -> Tri;
}

/// Verdict of the tables in force at a key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoverageVerdict {
    Covered { rule: String },
    Uncovered,
    Undecided { why: String },
}

pub trait Coverage {
    fn verdict(&self, key: &[i32]) -> CoverageVerdict;
}

impl Coverage for Dispatcher {
    fn verdict(&self, key: &[i32]) -> CoverageVerdict {
        match self.first_match(key) {
            Ok(Dispatch::Covered(r)) => CoverageVerdict::Covered {
                rule: format!("{}#{}", r.table, r.record),
            },
            Ok(Dispatch::Uncovered) => CoverageVerdict::Uncovered,
            Ok(other) => CoverageVerdict::Undecided {
                why: format!("{other:?}"),
            },
            Err(e) => CoverageVerdict::Undecided { why: e.to_string() },
        }
    }
}

/// No tables in force (everything uncovered).
pub struct NoTables;
impl Coverage for NoTables {
    fn verdict(&self, _key: &[i32]) -> CoverageVerdict {
        CoverageVerdict::Uncovered
    }
}

/// Limits of a sequential tree run (no clock: CPU is summed from the solver's answers).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeLimits {
    pub max_attempts: u32,
    pub max_cpu_ms: u64,
    pub max_items: u32,
}

impl Default for TreeLimits {
    fn default() -> Self {
        TreeLimits {
            max_attempts: 100_000,
            max_cpu_ms: u64::MAX,
            max_items: 100_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptSummary {
    pub item: ItemId,
    pub setting: String,
    pub outcome: String,
    pub cpu_ms: u64,
    pub candidates: usize,
}

/// Result of one item of a tree run.
#[derive(Clone, Debug)]
pub struct ItemReport {
    pub state: ItemState,
    pub disposition: Option<Disposition>,
}

#[derive(Clone, Debug, Default)]
pub struct TreeReport {
    pub items: Vec<ItemReport>,
    pub attempts: Vec<AttemptSummary>,
    pub cpu_ms: u64,
    /// Why the run stopped early (a limit), if it did.
    pub stopped: Option<String>,
}

impl TreeReport {
    /// Accepted cover members over all items (the rules a table would receive).
    pub fn accepted_rules(&self) -> Vec<(ItemId, crate::worklist::ladder::CoverMember)> {
        let mut out = Vec::new();
        for r in &self.items {
            let cover = match &r.disposition {
                Some(Disposition::Covered { cover }) | Some(Disposition::Split { cover, .. }) => {
                    cover
                }
                _ => continue,
            };
            for m in &cover.members {
                if !m.served.is_empty() {
                    out.push((r.state.item.id.clone(), m.clone()));
                }
            }
        }
        out
    }

    /// Terminal proposals over all items (each key once: the first exhausted ladder that proposed it).
    pub fn terminal_candidates(&self) -> Vec<(Key, LadderExhausted)> {
        let mut out: Vec<(Key, LadderExhausted)> = Vec::new();
        for r in &self.items {
            if let Some(Disposition::TerminalCandidates { keys, exhausted }) = &r.disposition {
                for k in keys {
                    if !out.iter().any(|(o, _)| o == k) {
                        out.push((k.clone(), exhausted.clone()));
                    }
                }
            }
        }
        out
    }
}

/// Sequential in-process driver of the ladder over a tree of items: each root, then depth first its PRIMARY
/// children, then its ALTERNATIVE children for the points the primaries' subtrees left unserved; one solve at a
/// time in wave order, verdicts through `facts` + `descent`, evidence through `prover`. Used by [`repair_key`] and by
/// the replay tests; the batch campaign drives [`next_steps`] itself.
#[allow(clippy::too_many_arguments)]
pub fn drive_tree(
    roots: Vec<Item>,
    params: &NodeParams,
    descent: &dyn Descent,
    solver: &mut dyn CaseSolver,
    facts: &mut dyn FactSource,
    prover: &mut dyn KeyProver,
    containment: Option<&mut dyn ContainmentCheck>,
    source: Option<&mut dyn CandidateSource>,
    limits: &TreeLimits,
) -> TreeReport {
    let mut d = Driver {
        source,
        params,
        descent,
        solver,
        facts,
        prover,
        containment,
        limits,
        report: TreeReport::default(),
        served: BTreeSet::new(),
        attempts: 0,
    };
    for r in roots {
        if d.report.stopped.is_some() {
            break;
        }
        d.process(r);
    }
    let served = d.served;
    let mut report = d.report;
    // a terminal proposal that a later (alternative) rule serves is not a terminal
    for r in &mut report.items {
        if let Some(Disposition::TerminalCandidates { keys, .. }) = &mut r.disposition {
            keys.retain(|k| !served.contains(k));
        }
    }
    report
}

struct Driver<'a, 'b, 's, 'f, 'p, 'c, 'q> {
    params: &'a NodeParams,
    descent: &'a dyn Descent,
    solver: &'b mut (dyn CaseSolver + 's),
    facts: &'b mut (dyn FactSource + 'f),
    prover: &'b mut (dyn KeyProver + 'p),
    containment: Option<&'b mut (dyn ContainmentCheck + 'c)>,
    source: Option<&'b mut (dyn CandidateSource + 'q)>,
    limits: &'a TreeLimits,
    report: TreeReport,
    served: BTreeSet<Key>,
    attempts: u32,
}

impl Driver<'_, '_, '_, '_, '_, '_, '_> {
    fn process(&mut self, mut item: Item) {
        if self.report.stopped.is_some() {
            return;
        }
        item.gap_points.retain(|k| !self.served.contains(k));
        item.weight = item.gap_points.len() as u64;
        if item.gap_points.is_empty() && item.kind != crate::worklist::items::ItemKind::Optimise {
            return;
        }
        if self.report.items.len() as u32 >= self.limits.max_items {
            self.report.stopped = Some(format!("item limit {}", self.limits.max_items));
            return;
        }
        self.report.items.push(ItemReport {
            state: ItemState::new(item),
            disposition: None,
        });
        let idx = self.report.items.len() - 1;
        loop {
            let st = &mut self.report.items[idx].state;
            match next_steps(st, self.params) {
                Next::Run(list) => {
                    if self.attempts >= self.limits.max_attempts {
                        self.report.stopped =
                            Some(format!("attempt limit {}", self.limits.max_attempts));
                        return;
                    }
                    if self.report.cpu_ms >= self.limits.max_cpu_ms {
                        self.report.stopped =
                            Some(format!("cpu limit {} ms", self.limits.max_cpu_ms));
                        return;
                    }
                    let a = &list[0];
                    let ans = self
                        .solver
                        .solve(&CaseRequest::from_attempt(&self.params.node, a), &a.budget);
                    self.attempts += 1;
                    self.report.cpu_ms = self.report.cpu_ms.saturating_add(ans.cpu_ms);
                    self.report.attempts.push(AttemptSummary {
                        item: a.item.clone(),
                        setting: a.setting.label(),
                        outcome: ans.outcome.tag().into(),
                        cpu_ms: ans.cpu_ms,
                        candidates: match &ans.outcome {
                            SolveOutcome::Rules(r) => r.len(),
                            _ => 0,
                        },
                    });
                    st.record_answer(a, ans);
                }
                Next::NeedVerdicts(reqs) => {
                    for r in reqs {
                        let cand = st.records[&r.attempt_key].candidates()[r.index].clone();
                        let (v, hits) = judge_points_with_hits(
                            &cand,
                            &r.keys,
                            self.facts,
                            self.descent,
                            &self.params.preferred,
                            &self.params.lines,
                        );
                        st.record_verdicts(&r.attempt_key, r.index, v);
                        st.record_preferred_hits(&r.attempt_key, r.index, hits);
                    }
                }
                Next::NeedEvidence(reqs) => {
                    for r in reqs {
                        let cand = st.records[&r.attempt_key].candidates()[r.index].clone();
                        let mut ev = self.prover.prove(&cand, &r.keys, r.level);
                        // a key the prover did not answer counts as Unknown (never PASS)
                        for k in &r.keys {
                            if !ev.iter().any(|e| &e.key == k) {
                                ev.push(KeyEvidence {
                                    key: k.clone(),
                                    level: None,
                                    failure: Some(crate::worklist::ladder::ProofFailure::Unknown(
                                        "no answer".into(),
                                    )),
                                });
                            }
                        }
                        st.record_evidence(&r.attempt_key, r.index, ev);
                    }
                }
                Next::NeedOffers => {
                    let (name, offers) = match self.source.as_deref_mut() {
                        Some(src) => {
                            let o = src.offer(&st.item, self.params);
                            (
                                src.name().to_string(),
                                o.into_iter().map(|c| c.candidate).collect(),
                            )
                        }
                        None => ("none".to_string(), Vec::new()),
                    };
                    st.record_offers(&name, offers);
                }
                Next::NeedContainment(reqs) => {
                    for r in reqs {
                        let cand = st.records[&r.attempt_key].candidates()[r.index].clone();
                        let t = match self.containment.as_deref_mut() {
                            Some(c) => c.contains(&cand, &r.old),
                            None => Tri::Unknown,
                        };
                        st.record_containment(&r.attempt_key, r.index, t);
                    }
                }
                Next::Done(d) => {
                    if let Disposition::Covered { cover } | Disposition::Split { cover, .. } = &d {
                        for m in &cover.members {
                            self.served.extend(m.served.iter().cloned());
                        }
                    }
                    let follow = match &d {
                        Disposition::Split {
                            children,
                            alternatives,
                            reserve,
                            ..
                        } => Some((children.clone(), alternatives.clone(), reserve.clone())),
                        _ => None,
                    };
                    self.report.items[idx].disposition = Some(d);
                    if let Some((children, alternatives, reserve)) = follow {
                        for c in children {
                            self.process(c);
                        }
                        for c in alternatives {
                            self.process(c);
                        }
                        // reserve members serve what the subtrees left (economy pruning, repair-library.md 4.2)
                        for mut m in reserve {
                            m.served.retain(|k| !self.served.contains(k));
                            if m.served.is_empty() {
                                continue;
                            }
                            self.served.extend(m.served.iter().cloned());
                            if let Some(Disposition::Split { cover, .. }) =
                                &mut self.report.items[idx].disposition
                            {
                                cover.members.push(m);
                            }
                        }
                    }
                    return;
                }
            }
        }
    }
}

/// Inputs of one live repair.
pub struct RepairInputs<'a> {
    pub params: &'a NodeParams,
    pub descent: &'a dyn Descent,
    pub key: Key,
    pub extra_targets: Vec<Key>,
    pub tables: &'a dyn Coverage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairBudget {
    pub max_attempts: u32,
    pub max_cpu_ms: u64,
    pub max_items: u32,
}

impl Default for RepairBudget {
    fn default() -> Self {
        RepairBudget {
            max_attempts: 200,
            max_cpu_ms: 3_600_000,
            max_items: 64,
        }
    }
}

/// An accepted repair rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedRule {
    pub digest: String,
    pub item: ItemId,
    pub case: CaseSpec,
    pub setting: String,
    pub served: Vec<Key>,
    pub rhs_terms: u64,
    /// This rule serves the repaired key.
    pub serves_key: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairOutcome {
    pub rules: Vec<AcceptedRule>,
    pub evidence: EvidenceLevel,
    pub attempts: Vec<AttemptSummary>,
    pub already_covered: bool,
}

/// Typed failures of a live repair; every one names what was tried.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepairFailure {
    NotImplemented,
    InvalidKey {
        why: String,
    },
    BudgetExhausted {
        attempts: Vec<AttemptSummary>,
    },
    NoRelation {
        attempts: Vec<AttemptSummary>,
    },
    NotAccepted {
        why: String,
        attempts: Vec<AttemptSummary>,
    },
    Terminal {
        key: Key,
        exhausted: LadderExhausted,
        attempts: Vec<AttemptSummary>,
    },
    /// The key is a PREFERRED terminal of the spec (P-4): kept unevaluated, no ladder effort spent.
    PreferredTerminal {
        key: Key,
    },
}

/// Repair ONE key (plan 3.9): if the tables in force cover it, nothing to do; else the whole face of the key is
/// driven through the ladder (seed search, children, point), sequentially, within the budget.
pub fn repair_key(
    inputs: &RepairInputs,
    budget: &RepairBudget,
    solver: &mut dyn CaseSolver,
    facts: &mut dyn FactSource,
    prover: &mut dyn KeyProver,
    source: Option<&mut dyn CandidateSource>,
) -> Result<RepairOutcome, RepairFailure> {
    let p = inputs.params;
    if inputs.key.len() != p.arity {
        return Err(RepairFailure::InvalidKey {
            why: format!("arity {} (node {})", inputs.key.len(), p.arity),
        });
    }
    if p.preferred.matches(&inputs.key, &p.lines) {
        return Err(RepairFailure::PreferredTerminal {
            key: inputs.key.clone(),
        });
    }
    match inputs.tables.verdict(&inputs.key) {
        CoverageVerdict::Covered { .. } => {
            return Ok(RepairOutcome {
                rules: vec![],
                evidence: p.min_evidence,
                attempts: vec![],
                already_covered: true,
            });
        }
        CoverageVerdict::Uncovered => {}
        CoverageVerdict::Undecided { why } => {
            return Err(RepairFailure::InvalidKey {
                why: format!("tables undecided at the key: {why}"),
            });
        }
    }
    let item = repair_item(&inputs.key, &inputs.extra_targets, p)
        .map_err(|e| RepairFailure::InvalidKey { why: e.to_string() })?;
    let limits = TreeLimits {
        max_attempts: budget.max_attempts,
        max_cpu_ms: budget.max_cpu_ms,
        max_items: budget.max_items,
    };
    let rep = drive_tree(
        vec![item],
        p,
        inputs.descent,
        solver,
        facts,
        prover,
        None,
        source,
        &limits,
    );
    let rules: Vec<AcceptedRule> = rep
        .accepted_rules()
        .into_iter()
        .map(|(item, m)| {
            let case = rep
                .items
                .iter()
                .find(|r| r.state.item.id == item)
                .map(|r| r.state.item.case.clone())
                .expect("item of a member");
            AcceptedRule {
                digest: m.digest.clone(),
                serves_key: m.served.contains(&inputs.key),
                item,
                case,
                setting: m.setting.label(),
                served: m.served.clone(),
                rhs_terms: m.rhs_terms,
            }
        })
        .collect();
    if rules.iter().any(|r| r.serves_key) {
        return Ok(RepairOutcome {
            rules,
            evidence: p.min_evidence,
            attempts: rep.attempts,
            already_covered: false,
        });
    }
    if let Some((k, ex)) = rep
        .terminal_candidates()
        .into_iter()
        .find(|(k, _)| *k == inputs.key)
    {
        return Err(RepairFailure::Terminal {
            key: k,
            exhausted: ex,
            attempts: rep.attempts,
        });
    }
    if rep.stopped.is_some() {
        return Err(RepairFailure::BudgetExhausted {
            attempts: rep.attempts,
        });
    }
    if rep.attempts.iter().all(|a| a.candidates == 0) {
        return Err(RepairFailure::NoRelation {
            attempts: rep.attempts,
        });
    }
    let why = rep
        .items
        .iter()
        .filter_map(|r| match &r.disposition {
            Some(Disposition::Attention { why }) => Some(why.clone()),
            _ => None,
        })
        .next()
        .unwrap_or_else(|| "no candidate was accepted at the key".into());
    Err(RepairFailure::NotAccepted {
        why,
        attempts: rep.attempts,
    })
}
