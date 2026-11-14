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
    Attempt, AttentionReason, CandidateRule, CoverMember, Disposition, DropReason, Exclusion,
    ExclusionReason, ExclusionTarget, ItemState, KeyEvidence, LadderExhausted, Next, PointFacts,
    ShadowVerdict, SolveAnswer, SolveOutcome, Tri, judge_points_with_hits, next_steps,
};
use crate::worklist::seed::{AttemptBudget, SeedSetting};
use crate::worklist::supplied::CandidateSource;

/// One engine call for ONE case under ONE seed setting (C-LIVE: the node driver in process, serialised).
pub trait CaseSolver {
    fn solve(&mut self, req: &CaseRequest, budget: &AttemptBudget) -> SolveAnswer;
    /// R48: solve ONE recipe at ONE integer key, deterministically (the trait face of the engine's `solve-recipe`
    /// verb, interfaces/coord-recipe-solve.md). Default: unsupported (a typed failure, never a pass).
    fn solve_recipe_at_key(
        &mut self,
        _recipe: &crate::worklist::recipe::Recipe,
        _key: &[i32],
    ) -> Result<crate::worklist::recipe::KeyRelation, crate::worklist::recipe::RecipeFailure> {
        Err(crate::worklist::recipe::RecipeFailure::Unsupported {
            why: "this solver has no recipe verb".into(),
        })
    }
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

/// The SHADOWING test of a new case BEFORE any search (R31 evidence 04:37Z): is the item's case fully shadowed by the
/// dispatch entries in force before it (`earlier`: the new rules of the run accepted so far, in rank order, with
/// their exclusions; the tables in force are the implementation's own), or empty? Only a PROVED answer drops the
/// case. [`PartitionShadowCheck`] is the implementation on `crate::partition::shadowed` / `is_empty`.
pub trait ShadowCheck {
    fn check(&mut self, item: &Item, earlier: &[(&CandidateRule, &[Exclusion])]) -> ShadowVerdict;
}

/// [`ShadowCheck`] through the exact domain tests of the partition module: the case is dropped when
/// `crate::partition::is_empty` proves its domain empty, or when `crate::partition::shadowed` proves that every key
/// of it is MATCHED (served or blocked) by an earlier entry: the tables in force (`tables`, in dispatch order) and
/// the run's earlier new rules WITHOUT exclusions (a rule with exclusions does not match at its excluded keys, so it
/// is left out: the test then proves less, never more).
pub struct PartitionShadowCheck<'a> {
    pub tables: &'a dyn crate::partition::DomainSource,
    pub frame: &'a crate::key::NodeFrame,
    pub budget: crate::loci::contain::ProofBudget,
}

impl ShadowCheck for PartitionShadowCheck<'_> {
    fn check(&mut self, item: &Item, earlier: &[(&CandidateRule, &[Exclusion])]) -> ShadowVerdict {
        use crate::loci::contain::{Containment, Emptiness, is_empty_with, shadowed_with};
        let Some(d) = item.case.case_domain(self.frame.mask()) else {
            return ShadowVerdict::Unknown {
                why: "case not expressible as a domain".into(),
            };
        };
        match is_empty_with(&d, self.frame, &self.budget) {
            Emptiness::ProvedEmpty { certificate } => {
                return ShadowVerdict::EmptyDomain { certificate };
            }
            Emptiness::NonEmpty { .. } | Emptiness::Unknown { .. } => {}
        }
        let mut list = crate::partition::DomainList::new(self.frame.arity());
        for i in 0..self.tables.len() {
            list.push(self.tables.rule_ref(i), self.tables.domain(i).clone());
        }
        for (r, ex) in earlier {
            if ex.is_empty() {
                let at = list.items.len();
                list.push(crate::cover::RuleRef::bare(usize::MAX, at, -1), r.domain.clone());
            }
        }
        let me = list.items.len();
        list.push(crate::cover::RuleRef::bare(usize::MAX, me, -2), d);
        match shadowed_with(&list, me, self.frame, &self.budget) {
            Containment::Proved { certificate } => ShadowVerdict::Shadowed { certificate },
            Containment::NotContained { .. } => ShadowVerdict::NotShadowed,
            Containment::Unknown { reason } => ShadowVerdict::Unknown { why: reason },
        }
    }
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
    /// Items the run did NOT process because it stopped (remaining roots, queued children and alternatives), with
    /// their full gap points: [`seal_tree`] excludes every returned rule at their gap points (`NotProcessed`,
    /// re-audit B C1), and a retry (e.g. with larger limits) re-drives exactly these.
    pub unprocessed: Vec<Item>,
    /// The safe-dispatch obligation of the returned rules (re-audit B m3; repair-library.md 6.1).
    pub dispatch: DispatchContract,
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

/// Complete the EFFECTIVE DOMAINS of every accepted member of a tree run (C1, both drivers call it ONCE over every
/// item whose rules are dispatched together, i.e. one call per round, never per tree: re-audit A m1; [`drive_tree`]
/// does): every gap point of ANY item of the run that lies in a member's written domain (first-match stage other
/// than "not selected") and at which the member is not credited becomes an exclusion (`NotCreditedHere`, unless an
/// item-level reason is already recorded); every gap point of a PENDING item (`pending`: items queued but not
/// processed because the run stopped, re-audit B C1) in the domain becomes `NotProcessed`; every HONOURED preferred
/// key of the spec (at most `preferred.bound(terminals_max)`, re-audit A m3) inside the domain becomes an exclusion
/// (`PreferredTerminal`); a child read from a member's POLE locus excludes the member on the child's locus
/// (`PoleLocusChild`, so the child is reached). Afterwards, at every gap point of the run (processed or pending),
/// every new rule that would be selected there is credited there: dispatch at the gap points is independent of the
/// order of the new rules, and a stopped run never leaves an unjudged rule in force at a key it did not reach.
pub fn seal_tree(items: &mut [ItemReport], pending: &[Item], params: &NodeParams) {
    let mut tree_points: BTreeSet<Key> = BTreeSet::new();
    for r in items.iter() {
        tree_points.extend(r.state.item.gap_points.iter().cloned());
    }
    let mut pending_points: BTreeMap<Key, ItemId> = BTreeMap::new();
    for it in pending {
        for k in &it.gap_points {
            if !tree_points.contains(k) {
                pending_points.entry(k.clone()).or_insert_with(|| it.id.clone());
            }
        }
    }
    let preferred = honoured_preferred_keys(params);
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
            for (k, it) in &pending_points {
                if !credited.contains(k) && rule.domain.applies(k).stage() != Stage::NotSelected {
                    crate::worklist::ladder::add_exclusion(
                        &mut m.exclusions,
                        Exclusion {
                            target: ExclusionTarget::Key(k.clone()),
                            reason: ExclusionReason::NotProcessed { item: it.clone() },
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

/// The preferred KEY entries of the spec that are HONOURED (the first `preferred.bound(terminals_max)` in key order;
/// the surplus is "preference not applied" and gets no exclusion: re-audit A m3).
fn honoured_preferred_keys(params: &NodeParams) -> Vec<Key> {
    let keys: Vec<Key> = params
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
    params
        .preferred
        .honoured(&keys, &params.lines, params.terminals_max)
        .into_iter()
        .collect()
}

/// Is `key` an HONOURED preferred terminal of the spec (within the bound, relative to the spec's key entries and
/// this key)?
pub fn honoured_preferred(params: &NodeParams, key: &[i32]) -> bool {
    if !params.preferred.matches(key, &params.lines) {
        return false;
    }
    let keys = honoured_preferred_keys(params);
    keys.iter().any(|k| k.as_slice() == key)
        || (!params
            .preferred
            .entries
            .iter()
            .any(|e| matches!(e, crate::worklist::items::PreferredEntry::Key(k) if k.as_slice() == key))
            && keys.len() < params.preferred.bound(params.terminals_max))
}

/// Sequential in-process driver of the ladder over a tree of items: each root, then depth first its PRIMARY
/// children (which receive the parent's reserve candidates as inherited wave-0 candidates), then its ALTERNATIVE
/// children for the points the primaries' subtrees left unserved; one solve at a time in wave order, verdicts
/// through `facts` + `descent`, evidence through `prover`; finally [`seal_tree`] over the processed AND the
/// unprocessed items (a stopped run, re-audit B C1). Bounded by `limits` (attempts, CPU, items, GLOBAL steps) and by
/// each item's own step bound. Used by [`repair_key`] and by the replay tests; the batch campaign drives
/// [`next_steps`] itself (and must call [`seal_tree`] on its finished round). No shadowing test (every case is
/// searched): see [`drive_tree_with`].
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
    drive_tree_with(
        roots,
        params,
        descent,
        solver,
        facts,
        prover,
        containment,
        source,
        None,
        limits,
    )
}

/// [`drive_tree`] with the SHADOWING test of every new case before its search (R31 evidence 04:37Z): `shadow` answers
/// [`Next::NeedShadowing`] against the run's earlier new rules (rank order, with their exclusions); without it the
/// answer is `Unknown` (the case is searched).
#[allow(clippy::too_many_arguments)]
pub fn drive_tree_with(
    roots: Vec<Item>,
    params: &NodeParams,
    descent: &dyn Descent,
    solver: &mut dyn CaseSolver,
    facts: &mut dyn FactSource,
    prover: &mut dyn KeyProver,
    containment: Option<&mut dyn ContainmentCheck>,
    source: Option<&mut dyn CandidateSource>,
    shadow: Option<&mut dyn ShadowCheck>,
    limits: &TreeLimits,
) -> TreeReport {
    let mut d = Driver {
        source,
        shadow,
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
    let pending = std::mem::take(&mut report.unprocessed);
    seal_tree(&mut report.items, &pending, params);
    report.unprocessed = pending;
    report
}

struct Driver<'a, 'b, 's, 'f, 'p, 'c, 'q, 'h> {
    params: &'a NodeParams,
    descent: &'a dyn Descent,
    solver: &'b mut (dyn CaseSolver + 's),
    facts: &'b mut (dyn FactSource + 'f),
    prover: &'b mut (dyn KeyProver + 'p),
    containment: Option<&'b mut (dyn ContainmentCheck + 'c)>,
    source: Option<&'b mut (dyn CandidateSource + 'q)>,
    shadow: Option<&'b mut (dyn ShadowCheck + 'h)>,
    limits: &'a TreeLimits,
    report: TreeReport,
    served: BTreeSet<Key>,
    attempts: u32,
    /// Report index of the first item with each id: an item reached again (same case through two planes) reuses the
    /// solver answers already recorded, so no attempt key is run twice in one tree (audit B M1).
    seen: BTreeMap<ItemId, usize>,
}

impl Driver<'_, '_, '_, '_, '_, '_, '_, '_> {
    fn process(&mut self, mut item: Item) {
        if self.report.stopped.is_some() {
            // never processed: kept (with its full gap points) for sealing and for a retry (re-audit B C1)
            self.report.unprocessed.push(item);
            return;
        }
        item.gap_points.retain(|k| !self.served.contains(k));
        item.weight = item.gap_points.len() as u64;
        if item.gap_points.is_empty() && item.kind != crate::worklist::items::ItemKind::Optimise {
            return;
        }
        if self.report.items.len() as u32 >= self.limits.max_items {
            self.report.stopped = Some(format!("item limit {}", self.limits.max_items));
            self.report.unprocessed.push(item);
            return;
        }
        let id = item.id.clone();
        let mut state = ItemState::new(item);
        if let Some(&j) = self.seen.get(&id) {
            for (k, r) in &self.report.items[j].state.records {
                if r.is_verdict() || !r.non_verdict.is_empty() || r.budget_limited.is_some() {
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
            let next = next_steps(&self.report.items[idx].state, self.params);
            match next {
                Next::NeedShadowing => {
                    let v = match self.shadow.as_deref_mut() {
                        Some(sc) => {
                            let earlier: Vec<(CandidateRule, Vec<Exclusion>)> = self
                                .report
                                .accepted_members()
                                .into_iter()
                                .filter_map(|(i, m)| {
                                    self.report
                                        .rule_of(i, &m)
                                        .map(|c| (c.clone(), m.exclusions.clone()))
                                })
                                .collect();
                            let refs: Vec<(&CandidateRule, &[Exclusion])> =
                                earlier.iter().map(|(c, e)| (c, e.as_slice())).collect();
                            sc.check(&self.report.items[idx].state.item, &refs)
                        }
                        None => ShadowVerdict::Unknown {
                            why: "no shadowing test configured".into(),
                        },
                    };
                    self.report.items[idx].state.record_shadowing(v);
                }
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
                    self.report.items[idx].state.record_answer(a, ans);
                }
                Next::NeedVerdicts(reqs) => {
                    let st = &mut self.report.items[idx].state;
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
                    let st = &mut self.report.items[idx].state;
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
                    let st = &mut self.report.items[idx].state;
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
                    let st = &mut self.report.items[idx].state;
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

/// THE SAFE-DISPATCH OBLIGATION of every rule the library returns (repair-library.md v1.3 section 6.1).
pub const SAFE_DISPATCH_STATEMENT: &str = "The new rules are safe only if (1) dispatched AFTER every table in force \
at the call (keys those tables cover keep their old winner), (2) in the order of their rank among themselves, and \
(3) with their exclusions honoured (key_excluded). Under (1)-(3) every key the library was asked about is either \
served by a rule credited there (coverage, descent under the node comparator, no hidden pole, identity at the \
required level) or not touched by the new rules. At keys of a rule's effective domain that were NOT asked about the \
library established nothing: the WINNER-DIFF stage is MANDATORY: the only exportable form of the new rules is \
WinnerChecked (PendingWinnerDiff::run), which checks d1-d4 at every key of the dispatch range where a new rule \
becomes the winner and excludes the rule where the check does not pass; outside the checked range the rules must \
not be dispatched.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchContract {
    /// Always true: the library assumes the new rules are dispatched after every table in force.
    pub after_tables_in_force: bool,
    /// Always true (v1.3): the new rules reach export / dispatch only through the winner-diff stage
    /// ([`PendingWinnerDiff::run`] -> [`WinnerChecked`]).
    #[serde(default = "yes")]
    pub winner_diff_required: bool,
    pub statement: String,
}

fn yes() -> bool {
    true
}

impl Default for DispatchContract {
    fn default() -> Self {
        DispatchContract {
            after_tables_in_force: true,
            winner_diff_required: true,
            statement: SAFE_DISPATCH_STATEMENT.into(),
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// THE WINNER-DIFF STAGE (mandatory; repair-library.md v1.3 section 6.1)

/// The per-key check of the winner-diff stage (plan 2.4.7 D3/D6; implemented by the closure / winner lane): at a key
/// where a new rule becomes the first-match WINNER without having been credited there, does the rule serve the key
/// (d1-d4 at the key, identity at the node's level)? Only `Tri::Pass` lets the rule win there.
pub trait WinnerCheck {
    fn check(&mut self, rule: &CandidateRule, key: &[i32]) -> Tri;
}

/// One new rule as the winner-diff stage sees it (fields private).
#[derive(Clone, Debug)]
pub struct NewRule {
    rule: CandidateRule,
    exclusions: Vec<Exclusion>,
    credited: BTreeSet<Key>,
    rank: u32,
    item: ItemId,
}

impl NewRule {
    pub fn rule(&self) -> &CandidateRule {
        &self.rule
    }
    pub fn exclusions(&self) -> &[Exclusion] {
        &self.exclusions
    }
    pub fn credited(&self) -> &BTreeSet<Key> {
        &self.credited
    }
    pub fn rank(&self) -> u32 {
        self.rank
    }
    pub fn item(&self) -> &ItemId {
        &self.item
    }
}

/// The sealed rules of a run AWAITING the winner-diff stage: no accessor hands them out for dispatch; the only way
/// on is [`Self::run`] (re-audit A notes / re-audit B section 4: 16-18 % of the dispatches at keys never asked about
/// were wrong when the stage was skipped).
#[derive(Clone, Debug)]
pub struct PendingWinnerDiff {
    rules: Vec<NewRule>,
    stopped: bool,
}

/// Outcome of the winner-diff stage at one key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WinnerDiffCounts {
    /// Keys of the range dispatched.
    pub keys: u64,
    /// Keys the tables in force decide (covered / undecided): the new rules do not win there.
    pub tables: u64,
    /// Keys won by a new rule credited there.
    pub credited: u64,
    /// Keys won by a new rule NOT credited there whose per-key check passed.
    pub checked: u64,
    /// (key, rule) pairs where the check failed / was unknown: the rule was excluded at the key.
    pub excluded: u64,
    /// Keys where a new rule is selected but blocked (pole / undecided): left open.
    pub blocked: u64,
    /// Keys no new rule reaches.
    pub untouched: u64,
}

/// The ONLY exportable form of the library's new rules: every key of `range` was dispatched (tables first, then the
/// new rules in rank order with exclusions) and every new winner either credited there or checked; failed checks
/// became `WinnerCheckFailed` exclusions. Fields private; constructed only by [`PendingWinnerDiff::run`]. The rules
/// may be dispatched only inside the checked range.
#[derive(Clone, Debug)]
pub struct WinnerChecked {
    rules: Vec<NewRule>,
    range: Vec<Key>,
    counts: WinnerDiffCounts,
    stopped: bool,
}

impl WinnerChecked {
    /// The rules in rank order, with their final exclusions.
    pub fn rules(&self) -> &[NewRule] {
        &self.rules
    }
    /// First-match view: (rule, exclusions) in rank order.
    pub fn dispatch_list(&self) -> Vec<(&CandidateRule, &[Exclusion])> {
        self.rules
            .iter()
            .map(|r| (&r.rule, r.exclusions.as_slice()))
            .collect()
    }
    /// The keys the stage checked (sorted, unique): the rules are valid ONLY there.
    pub fn range(&self) -> &[Key] {
        &self.range
    }
    pub fn counts(&self) -> &WinnerDiffCounts {
        &self.counts
    }
    /// The run that produced the rules stopped at a limit (its unprocessed items carry `NotProcessed` exclusions).
    pub fn from_stopped_run(&self) -> bool {
        self.stopped
    }
}

impl PendingWinnerDiff {
    pub fn len(&self) -> usize {
        self.rules.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
    /// THE WINNER-DIFF STAGE over `range` (the keys at which the new rules are to be dispatched: the node's forward
    /// closure, plan 2.4.7 D6): at every key the tables in force come first; where they leave the key uncovered, the
    /// first new rule (rank order, exclusions honoured) that wins it must be credited there, or pass `check`;
    /// otherwise the rule is EXCLUDED at the key and dispatch moves on (at most one exclusion per rule and key, so
    /// the loop ends).
    pub fn run(
        self,
        range: &[Key],
        tables: &dyn Coverage,
        check: &mut dyn WinnerCheck,
    ) -> WinnerChecked {
        let mut rules = self.rules;
        let mut keys: Vec<Key> = range.to_vec();
        keys.sort();
        keys.dedup();
        let mut c = WinnerDiffCounts::default();
        for k in &keys {
            c.keys += 1;
            if !matches!(tables.verdict(k), CoverageVerdict::Uncovered) {
                c.tables += 1;
                continue;
            }
            loop {
                let fm = {
                    let list: Vec<(&CandidateRule, &[Exclusion])> = rules
                        .iter()
                        .map(|r| (&r.rule, r.exclusions.as_slice()))
                        .collect();
                    first_match_with_exclusions(&list, k)
                };
                match fm {
                    FirstMatch::Rule(i) if rules[i].credited.contains(k) => {
                        c.credited += 1;
                        break;
                    }
                    FirstMatch::Rule(i) => match check.check(&rules[i].rule, k) {
                        Tri::Pass => {
                            c.checked += 1;
                            break;
                        }
                        t => {
                            c.excluded += 1;
                            crate::worklist::ladder::add_exclusion(
                                &mut rules[i].exclusions,
                                Exclusion {
                                    target: ExclusionTarget::Key(k.clone()),
                                    reason: ExclusionReason::WinnerCheckFailed {
                                        why: format!("{t:?}"),
                                    },
                                },
                            );
                            rules[i].exclusions.sort();
                        }
                    },
                    FirstMatch::Blocked(_) => {
                        c.blocked += 1;
                        break;
                    }
                    FirstMatch::None => {
                        c.untouched += 1;
                        break;
                    }
                }
            }
        }
        WinnerChecked {
            rules,
            range: keys,
            counts: c,
            stopped: self.stopped,
        }
    }
}

impl TreeReport {
    /// The run's accepted rules (rank order, sealed exclusions) handed to the MANDATORY winner-diff stage. Call it
    /// on a SEALED report ([`drive_tree`] seals; a batch driver calls [`seal_tree`] first).
    pub fn pending_winner_diff(&self) -> PendingWinnerDiff {
        let mut rules = Vec::new();
        for (rank, (i, m)) in self.accepted_members().into_iter().enumerate() {
            if let Some(c) = self.rule_of(i, &m) {
                rules.push(NewRule {
                    rule: c.clone(),
                    exclusions: m.exclusions.clone(),
                    credited: m.credited.iter().cloned().collect(),
                    rank: rank as u32,
                    item: self.items[i].state.item.id.clone(),
                });
            }
        }
        PendingWinnerDiff {
            rules,
            stopped: self.stopped.is_some(),
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
    /// The keys at which the returned rules may be used AT ONCE (the repaired key and the extra targets: every rule
    /// was checked there); anywhere else only after the winner-diff stage (`pending`).
    #[serde(default)]
    pub usable_at: Vec<Key>,
    /// The returned rules awaiting the winner-diff stage for every other key (not serialised).
    #[serde(skip)]
    pub pending: Option<PendingWinnerDiff>,
}

impl PartialEq for PendingWinnerDiff {
    fn eq(&self, o: &Self) -> bool {
        self.stopped == o.stopped
            && self.rules.len() == o.rules.len()
            && self
                .rules
                .iter()
                .zip(&o.rules)
                .all(|(a, b)| a.rule.digest == b.rule.digest && a.exclusions == b.exclusions && a.rank == b.rank)
    }
}
impl Eq for PendingWinnerDiff {}

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
    /// The key is an HONOURED preferred terminal of the spec (P-4, within `max_preferred`): kept unevaluated, no
    /// ladder effort spent.
    PreferredTerminal {
        key: Key,
    },
    /// The key's case was dropped before search (shadowed by earlier dispatch entries, or an empty domain; R31
    /// evidence 04:37Z).
    Dropped {
        reason: DropReason,
        attempts: Vec<AttemptSummary>,
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
    if honoured_preferred(p, &inputs.key) {
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
                usable_at: vec![],
                pending: None,
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
        let pending = Some(rep.pending_winner_diff());
        return Ok(RepairOutcome {
            rules,
            evidence: p.min_evidence,
            attempts: rep.attempts,
            already_covered: false,
            dispatch: DispatchContract::default(),
            usable_at: asked,
            pending,
        });
    }
    if let Some(reason) = rep.items.iter().find_map(|r| match &r.disposition {
        Some(Disposition::Dropped { reason, keys }) if keys.contains(&inputs.key) => {
            Some(reason.clone())
        }
        _ => None,
    }) {
        return Err(RepairFailure::Dropped {
            reason,
            attempts: rep.attempts,
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
