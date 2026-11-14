//! Live repair as a library (plan 3.9, R22; interface `interfaces/repair-library.md`): the traits through which the
//! engine (lane C-LIVE) or the campaign (lane I-INT) plug in the case solver, the per-key facts, the key prover and
//! the tables in force; a sequential in-process driver of the ladder ([`drive_tree`]); and [`repair_key`].
//! No clock, no files, no global state: budgets are arguments; CPU is what the solver reports.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::cover::{Dispatch, Dispatcher};
use crate::domain::Stage;
use crate::order::NodeComparator;
use crate::worklist::items::{CaseSpec, EvidenceLevel, Item, ItemId, Key, NodeParams, repair_item};
use crate::worklist::ladder::{
    Attempt, AttentionReason, CandidateRule, CoverMember, Disposition, Exclusion, ExclusionReason,
    ExclusionTarget, ItemState, KeyEvidence, LadderExhausted, Next, PointFacts, SolveAnswer,
    SolveOutcome, Tri, judge_points_with_hits, next_steps,
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

/// Limits of a sequential tree run (no clock: CPU is summed from the solver's answers). `max_steps` is the GLOBAL
/// bound on driver iterations (every `next_steps` call answered, over all items): the run stops (`stopped`) when it
/// is reached, whatever the trait answers (C2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeLimits {
    pub max_attempts: u32,
    pub max_cpu_ms: u64,
    pub max_items: u32,
    #[serde(default = "default_tree_steps")]
    pub max_steps: u64,
}

fn default_tree_steps() -> u64 {
    1_000_000
}

impl Default for TreeLimits {
    fn default() -> Self {
        TreeLimits {
            max_attempts: 100_000,
            max_cpu_ms: u64::MAX,
            max_items: 100_000,
            max_steps: default_tree_steps(),
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
    /// Driver iterations (answered `next_steps` calls) over the whole run.
    pub steps: u64,
    /// Why the run stopped early (a limit), if it did.
    pub stopped: Option<String>,
}

impl TreeReport {
    /// Accepted cover members over all items (the rules a table would receive), in DISPATCH RANK order: items in
    /// the order they were processed (a parent before its children), members in cover order inside an item. The
    /// rank is the position in this list.
    pub fn accepted_rules(&self) -> Vec<(ItemId, CoverMember)> {
        self.accepted_members()
            .into_iter()
            .map(|(i, m)| (self.items[i].state.item.id.clone(), m))
            .collect()
    }

    /// [`Self::accepted_rules`] with the INDEX of the item report (item ids are content addresses: two items of one
    /// run can share an id, the index cannot).
    pub fn accepted_members(&self) -> Vec<(usize, CoverMember)> {
        let mut out = Vec::new();
        for (i, r) in self.items.iter().enumerate() {
            let cover = match &r.disposition {
                Some(Disposition::Covered { cover }) | Some(Disposition::Split { cover, .. }) => {
                    cover
                }
                _ => continue,
            };
            for m in &cover.members {
                if !m.served.is_empty() {
                    out.push((i, m.clone()));
                }
            }
        }
        out
    }

    /// The candidate rule (with its domain) of an accepted member of item report `index`.
    pub fn rule_of(&self, index: usize, m: &CoverMember) -> Option<&CandidateRule> {
        self.items
            .get(index)
            .and_then(|r| r.state.records.get(&m.attempt_key))
            .and_then(|rec| rec.candidates().get(m.index))
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

/// Outcome of first-match dispatch over NEW rules with their exclusions (the tables in force come first and are
/// not part of this list).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FirstMatch {
    /// Index of the rule that is selected and applies.
    Rule(usize),
    /// A rule is selected but blocked / undecided at the key (index): the key stays open.
    Blocked(usize),
    None,
}

/// First-match dispatch in the given order, skipping a rule at the keys / loci of its exclusions (the semantics
/// the library's outputs assume; the engine's `key_excluded` records).
pub fn first_match_with_exclusions(
    rules: &[(&CandidateRule, &[Exclusion])],
    key: &[i32],
) -> FirstMatch {
    for (i, (r, ex)) in rules.iter().enumerate() {
        if ex.iter().any(|e| e.target.holds(key)) {
            continue;
        }
        match r.domain.applies(key).stage() {
            Stage::NotSelected => continue,
            Stage::Applies => return FirstMatch::Rule(i),
            Stage::Blocked | Stage::Undecided => return FirstMatch::Blocked(i),
        }
    }
    FirstMatch::None
}

/// Complete the EFFECTIVE DOMAINS of every accepted member of a tree run (C1, both drivers call it once the tree is
/// done; [`drive_tree`] does): every gap point of ANY item of the run that lies in a member's written domain
/// (first-match stage other than "not selected") and at which the member is not credited becomes an exclusion
/// (`NotCreditedHere`, unless an item-level reason is already recorded); every preferred KEY of the spec inside the
/// domain becomes an exclusion (`PreferredTerminal`); a child read from a member's POLE locus excludes the member on
/// the child's locus (`PoleLocusChild`, so the child is reached). Afterwards, at every gap point of the run, every
/// new rule that would be selected there is credited there: dispatch at the gap points is independent of the order
/// of the new rules.
pub fn seal_tree(items: &mut [ItemReport], params: &NodeParams) {
    let mut tree_points: BTreeSet<Key> = BTreeSet::new();
    for r in items.iter() {
        tree_points.extend(r.state.item.gap_points.iter().cloned());
    }
    let preferred: Vec<Key> = params
        .preferred
        .entries
        .iter()
        .filter_map(|e| match e {
            crate::worklist::items::PreferredEntry::Key(k) if k.len() == params.arity => {
                Some(k.clone())
            }
            _ => None,
        })
        .collect();
    for r in items.iter_mut() {
        let ItemReport { state, disposition } = r;
        let (cover, children) = match disposition {
            Some(Disposition::Covered { cover }) => (cover, Vec::new()),
            Some(Disposition::Split {
                cover, children, ..
            }) => (cover, children.clone()),
            _ => continue,
        };
        for m in &mut cover.members {
            let Some(rule) = state
                .records
                .get(&m.attempt_key)
                .and_then(|rec| rec.candidates().get(m.index))
            else {
                continue;
            };
            let credited: BTreeSet<&Key> = m.credited.iter().collect();
            for k in &tree_points {
                if credited.contains(k) {
                    continue;
                }
                if rule.domain.applies(k).stage() != Stage::NotSelected {
                    crate::worklist::ladder::add_exclusion(
                        &mut m.exclusions,
                        Exclusion {
                            target: ExclusionTarget::Key(k.clone()),
                            reason: ExclusionReason::NotCreditedHere,
                        },
                    );
                }
            }
            for k in &preferred {
                if !credited.contains(k) && rule.domain.applies(k).stage() != Stage::NotSelected {
                    crate::worklist::ladder::add_exclusion(
                        &mut m.exclusions,
                        Exclusion {
                            target: ExclusionTarget::Key(k.clone()),
                            reason: ExclusionReason::PreferredTerminal,
                        },
                    );
                }
            }
            for c in &children {
                if let crate::worklist::items::ChildOrigin::ExceptionBranch { branches, .. } =
                    &c.origin
                    && branches
                        .iter()
                        .any(|b| b.starts_with(&format!("{}#pole", rule.digest)))
                {
                    let mut eqs: Vec<crate::worklist::affine::AffineEq> = c
                        .case
                        .fixed
                        .iter()
                        .map(|(&s, &v)| crate::worklist::affine::AffineEq::fixed(s, v))
                        .collect();
                    eqs.extend(c.case.equations.iter().cloned());
                    crate::worklist::ladder::add_exclusion(
                        &mut m.exclusions,
                        Exclusion {
                            target: ExclusionTarget::Locus { equations: eqs },
                            reason: ExclusionReason::PoleLocusChild {
                                child: c.id.clone(),
                            },
                        },
                    );
                }
            }
            m.exclusions.sort();
        }
    }
}

/// Sequential in-process driver of the ladder over a tree of items: each root, then depth first its PRIMARY
/// children (which receive the parent's reserve candidates as inherited wave-0 candidates), then its ALTERNATIVE
/// children for the points the primaries' subtrees left unserved; one solve at a time in wave order, verdicts
/// through `facts` + `descent`, evidence through `prover`; finally [`seal_tree`]. Bounded by `limits` (attempts,
/// CPU, items, GLOBAL steps) and by each item's own step bound. Used by [`repair_key`] and by the replay tests; the
/// batch campaign drives [`next_steps`] itself (and must call [`seal_tree`] on its finished tree).
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
        seen: BTreeMap::new(),
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
    seal_tree(&mut report.items, params);
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
    /// Report index of the first item with each id: an item reached again (same case through two planes) reuses the
    /// solver answers already recorded, so no attempt key is run twice in one tree (audit B M1).
    seen: BTreeMap<ItemId, usize>,
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
        let id = item.id.clone();
        let mut state = ItemState::new(item);
        if let Some(&j) = self.seen.get(&id) {
            for (k, r) in &self.report.items[j].state.records {
                if r.is_verdict() || !r.non_verdict.is_empty() {
                    state.records.entry(k.clone()).or_insert_with(|| r.clone());
                }
            }
        }
        self.report.items.push(ItemReport {
            state,
            disposition: None,
        });
        let idx = self.report.items.len() - 1;
        self.seen.insert(id, idx); // the latest instance holds every answer so far
        loop {
            if self.report.steps >= self.limits.max_steps {
                self.report.stopped = Some(format!("step limit {}", self.limits.max_steps));
                return;
            }
            self.report.steps += 1;
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
                    let Some(a) = list.first() else {
                        self.report.stopped = Some("empty attempt list".into());
                        return;
                    };
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
                        // only the requested keys are kept; a requested key the prover did not answer is Unknown
                        ev.retain(|e| r.keys.contains(&e.key));
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
                            ..
                        } => Some((children.clone(), alternatives.clone())),
                        _ => None,
                    };
                    self.report.items[idx].disposition = Some(d);
                    if let Some((children, alternatives)) = follow {
                        for c in children {
                            self.process(c);
                        }
                        for c in alternatives {
                            self.process(c);
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

/// Budget of one live repair: solver calls, reported CPU, items, and the GLOBAL step bound (driver iterations).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairBudget {
    pub max_attempts: u32,
    pub max_cpu_ms: u64,
    pub max_items: u32,
    #[serde(default = "default_repair_steps")]
    pub max_steps: u64,
}

fn default_repair_steps() -> u64 {
    20_000
}

impl Default for RepairBudget {
    fn default() -> Self {
        RepairBudget {
            max_attempts: 200,
            max_cpu_ms: 3_600_000,
            max_items: 64,
            max_steps: default_repair_steps(),
        }
    }
}

/// An accepted repair rule with its EFFECTIVE DOMAIN (written case minus `exclusions`) and dispatch rank.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedRule {
    pub digest: String,
    pub item: ItemId,
    /// The rule's WRITTEN case.
    pub case: CaseSpec,
    pub setting: String,
    pub served: Vec<Key>,
    /// Keys asked about where the rule is credited (verdict covered + evidence at the required level).
    pub credited: Vec<Key>,
    /// The rule must not be dispatched at these keys / loci (key_excluded records).
    pub exclusions: Vec<Exclusion>,
    /// Position among the NEW rules of this call (0 first), all of them AFTER the tables in force.
    pub rank: u32,
    pub rhs_terms: u64,
    /// This rule serves the repaired key.
    pub serves_key: bool,
}

/// THE SAFE-DISPATCH OBLIGATION of every rule the library returns (repair-library.md v1.2 section 6.1).
pub const SAFE_DISPATCH_STATEMENT: &str = "The new rules are safe only if (1) dispatched AFTER every table in force \
at the call (keys those tables cover keep their old winner), (2) in the order of their rank among themselves, and \
(3) with their exclusions honoured (key_excluded). Under (1)-(3) every key the library was asked about is either \
served by a rule credited there (coverage, descent under the node comparator, no hidden pole, identity at the \
required level) or not touched by the new rules. At keys of a rule's effective domain that were NOT asked about the \
library established nothing: the closure / winner-diff stage (plan 2.4.7 D3/D6) must check d1-d4 at every key where \
a new rule becomes the winner, or the caller must not dispatch the rule there.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchContract {
    /// Always true: the library assumes the new rules are dispatched after every table in force.
    pub after_tables_in_force: bool,
    pub statement: String,
}

impl Default for DispatchContract {
    fn default() -> Self {
        DispatchContract {
            after_tables_in_force: true,
            statement: SAFE_DISPATCH_STATEMENT.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairOutcome {
    pub rules: Vec<AcceptedRule>,
    pub evidence: EvidenceLevel,
    pub attempts: Vec<AttemptSummary>,
    pub already_covered: bool,
    pub dispatch: DispatchContract,
}

/// Typed failures of a live repair; every one names what was tried. Serialise only: `Terminal` carries the
/// unforgeable exhaustion token (deserialise its `LadderExhaustedRecord` and re-verify instead).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
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
    /// The ladder could not decide (crashes, kills, refusals, step bound, interrupts): a human is needed; never a
    /// terminal (C3).
    Attention {
        reason: AttentionReason,
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
/// driven through the ladder (seed search, children, point), sequentially, within the budget. On success the rule
/// serving the key is returned with every other rule of the run, each with its exclusions and rank, and the
/// [`DispatchContract`]; the result is checked by first-match dispatch at every key asked about.
#[allow(clippy::result_large_err)]
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
                dispatch: DispatchContract::default(),
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
    let asked: Vec<Key> = item.gap_points.clone();
    let limits = TreeLimits {
        max_attempts: budget.max_attempts,
        max_cpu_ms: budget.max_cpu_ms,
        max_items: budget.max_items,
        max_steps: budget.max_steps,
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
    let members = rep.accepted_members();
    let mut rules: Vec<AcceptedRule> = Vec::new();
    let mut cands: Vec<(&CandidateRule, &[Exclusion])> = Vec::new();
    for (rank, (idx, m)) in members.iter().enumerate() {
        let item = &rep.items[*idx].state.item.id;
        let Some(c) = rep.rule_of(*idx, m) else {
            return Err(RepairFailure::NotAccepted {
                why: format!("internal: no candidate for member {}", m.digest),
                attempts: rep.attempts.clone(),
            });
        };
        cands.push((c, m.exclusions.as_slice()));
        rules.push(AcceptedRule {
            digest: m.digest.clone(),
            serves_key: m.served.contains(&inputs.key),
            item: item.clone(),
            case: m.case.clone(),
            setting: m.setting.label(),
            served: m.served.clone(),
            credited: m.credited.clone(),
            exclusions: m.exclusions.clone(),
            rank: rank as u32,
            rhs_terms: m.rhs_terms,
        });
    }
    if rules.iter().any(|r| r.serves_key) {
        // self-check: at every key asked about, first match over the new rules (in rank order, with exclusions)
        // is a rule credited there, or nothing; at the repaired key it must be a credited rule
        for k in &asked {
            match first_match_with_exclusions(&cands, k) {
                FirstMatch::Rule(i) if rules[i].credited.contains(k) => {}
                FirstMatch::None if k != &inputs.key => {}
                other => {
                    return Err(RepairFailure::NotAccepted {
                        why: format!("dispatch check failed at {k:?}: {other:?}"),
                        attempts: rep.attempts,
                    });
                }
            }
        }
        return Ok(RepairOutcome {
            rules,
            evidence: p.min_evidence,
            attempts: rep.attempts,
            already_covered: false,
            dispatch: DispatchContract::default(),
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
    if let Some((reason, why)) = rep.items.iter().find_map(|r| match &r.disposition {
        Some(Disposition::Attention { reason, why }) => Some((reason.clone(), why.clone())),
        _ => None,
    }) {
        return Err(RepairFailure::Attention {
            reason,
            why,
            attempts: rep.attempts,
        });
    }
    if rep.attempts.iter().all(|a| a.candidates == 0) {
        return Err(RepairFailure::NoRelation {
            attempts: rep.attempts,
        });
    }
    Err(RepairFailure::NotAccepted {
        why: "no candidate was accepted at the key".into(),
        attempts: rep.attempts,
    })
}
