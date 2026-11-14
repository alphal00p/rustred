//! The ladder of one item as a pure state machine (plan 2.4.4 transition table, D-35, D-43; 2.4.13 grid; 2.4.14 (b)
//! selection): [`next_steps`] reads the item's state (its attempt history keyed by attempt key, the point verdicts
//! and the key evidence the driver recorded) and returns the next attempts, the next facts it needs, or the final
//! disposition. Acceptance of a candidate at a gap point is [`judge_point`] (d1-d4 through the core domain, the
//! driver's facts and the FULL node comparator).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::children::{derive_children, excepting_branch};
use super::items::{EvidenceLevel, Item, ItemId, ItemKind, Key, NodeParams};
use super::seed::{AttemptBudget, SeedSetting, attempt_budget, engine_seed_args, seed_space};
use crate::domain::{CaseDomain, Verdict};
use crate::economy::SelectionScore;
use crate::repair::{Descent, FactSource};

/// Rung of an attempt: the seed search (`L1s`, contains L0's target attempt) on a case, or its point form (L4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rung {
    SeedSearch,
    Point,
}

/// One engine call to make.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    pub item: ItemId,
    /// "<item id>|<setting canonical key>": never re-run once answered (operator / infra interrupts excepted).
    pub key: String,
    pub case: super::items::CaseSpec,
    pub setting: SeedSetting,
    pub rung: Rung,
    /// 0 = depth-2 wave, 1 = depth-3 wave.
    pub wave: u8,
    pub budget: AttemptBudget,
    pub seed_args: Vec<String>,
    /// 1 + interrupts so far (a new attempt number for the same key after an interrupt).
    pub attempt_no: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Interruption {
    Operator,
    Infra,
}

/// A rule returned by the case solver (the driver builds `domain` from the rule record: case, exception branches,
/// pole and witness loci, elision).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateRule {
    pub domain: CaseDomain,
    pub digest: String,
    pub rhs_terms: u64,
    pub monomials: Option<u64>,
    pub max_degree: Option<u32>,
    pub bytes: Option<u64>,
    pub rows: Option<u64>,
    /// Informative only (the comparator decides descent).
    pub tie_break_only: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolveOutcome {
    Rules(Vec<CandidateRule>),
    /// No relation within the shells of the setting.
    NoRelation,
    BudgetStop { time_decided: bool },
    RefusedHeavy { reason: String },
    ProfileRefused { reason: String },
    KilledMemory,
    Interrupted { by: Interruption },
    Error(String),
}

impl SolveOutcome {
    pub fn tag(&self) -> &'static str {
        match self {
            SolveOutcome::Rules(_) => "rules",
            SolveOutcome::NoRelation => "no_relation",
            SolveOutcome::BudgetStop { .. } => "budget_stop",
            SolveOutcome::RefusedHeavy { .. } => "refused_heavy",
            SolveOutcome::ProfileRefused { .. } => "profile_refused",
            SolveOutcome::KilledMemory => "killed_memory",
            SolveOutcome::Interrupted { .. } => "interrupted",
            SolveOutcome::Error(_) => "error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolveAnswer {
    pub outcome: SolveOutcome,
    pub cpu_ms: u64,
    pub rows: Option<u64>,
}

/// History entry of one attempt key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptRecord {
    pub setting: SeedSetting,
    pub wave: u8,
    pub answer: Option<SolveAnswer>,
    pub operator_interrupts: u32,
    pub infra_interrupts: u32,
    pub cpu_ms_total: u64,
}

impl AttemptRecord {
    pub fn is_final(&self) -> bool {
        self.answer.is_some()
    }
    pub fn candidates(&self) -> &[CandidateRule] {
        match &self.answer {
            Some(SolveAnswer {
                outcome: SolveOutcome::Rules(r),
                ..
            }) => r,
            _ => &[],
        }
    }
}

/// Per-key facts of a candidate (rr_proof instantiate as a library, EC-2; R38: regularity per key, exact, no
/// factoring of denominators).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointFacts {
    pub regular: Tri,
    /// Top-sector RHS keys whose coefficient is not identically zero at the key; `None` = unknown.
    pub rhs_top: Option<Vec<Key>>,
    pub exceptions_complete: Tri,
    pub note: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tri {
    Pass,
    Fail,
    Unknown,
}

/// Verdict of a candidate at one gap point (only `Covered` credits).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointVerdict {
    Covered,
    NotInCase,
    OnException { branch: usize },
    OnPole,
    Elided,
    /// d2 failed with no exception branch there: the rule's exceptions are incomplete (rejects the candidate).
    HiddenPole,
    NotDescending { rhs: Key },
    Unknown { why: String },
}

impl PointVerdict {
    pub fn covered(&self) -> bool {
        matches!(self, PointVerdict::Covered)
    }
}

/// d1-d4 at one key (plan 2.4.5): d1 through the core domain (fixed, chart, equations, every exception branch, pole
/// loci, elision fail-closed), d2 and d4 from the facts, d3 with the FULL node comparator on every top RHS key.
pub fn judge_point(rule: &CandidateRule, key: &[i32], facts: &PointFacts, descent: &dyn Descent) -> PointVerdict {
    match rule.domain.applies(key) {
        Verdict::Applies => {}
        Verdict::OnException { branch } => return PointVerdict::OnException { branch },
        Verdict::OnPole { .. } => return PointVerdict::OnPole,
        Verdict::Elided => return PointVerdict::Elided,
        Verdict::Error(e) => return PointVerdict::Unknown { why: e.to_string() },
        _ => return PointVerdict::NotInCase,
    }
    match facts.regular {
        Tri::Pass => {}
        Tri::Fail => return PointVerdict::HiddenPole,
        Tri::Unknown => {
            return PointVerdict::Unknown {
                why: "regularity unknown".into(),
            };
        }
    }
    let Some(rhs) = &facts.rhs_top else {
        return PointVerdict::Unknown {
            why: "rhs keys unknown".into(),
        };
    };
    for r in rhs {
        if !descent.lower(r, key) {
            return PointVerdict::NotDescending { rhs: r.clone() };
        }
    }
    match facts.exceptions_complete {
        Tri::Pass => PointVerdict::Covered,
        Tri::Fail => PointVerdict::HiddenPole,
        Tri::Unknown => PointVerdict::Unknown {
            why: "exception completeness unknown (d4)".into(),
        },
    }
}

/// Judge one candidate at a list of keys through a fact source (driver helper).
pub fn judge_points(
    rule: &CandidateRule,
    keys: &[Key],
    facts: &mut dyn FactSource,
    descent: &dyn Descent,
) -> Vec<PointVerdict> {
    keys.iter()
        .map(|k| {
            // d1 first: facts are only fetched for keys inside the domain
            match rule.domain.applies(k) {
                Verdict::Applies => {
                    let f = facts.facts(rule, k);
                    judge_point(rule, k, &f, descent)
                }
                _ => judge_point(
                    rule,
                    k,
                    &PointFacts {
                        regular: Tri::Unknown,
                        rhs_top: None,
                        exceptions_complete: Tri::Unknown,
                        note: None,
                    },
                    descent,
                ),
            }
        })
        .collect()
}

/// Proof failures of the key prover (A s.2: kept apart, never PASS).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofFailure {
    NonMembership,
    Fail,
    Hidden,
    Irregular,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvidence {
    pub key: Key,
    pub level: Option<EvidenceLevel>,
    pub failure: Option<ProofFailure>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerdictRequest {
    pub attempt_key: String,
    pub index: usize,
    pub keys: Vec<Key>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceRequest {
    pub attempt_key: String,
    pub index: usize,
    pub keys: Vec<Key>,
    pub level: EvidenceLevel,
}

/// The token that a POINT's ladder failed completely (the only way to make one; terminals.rs requires it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LadderExhausted {
    item: ItemId,
    key: Key,
    attempts: u32,
    depth3_tried: bool,
    depth3_unfloored_tried: bool,
}

impl LadderExhausted {
    pub fn item(&self) -> &ItemId {
        &self.item
    }
    pub fn key(&self) -> &[i32] {
        &self.key
    }
    pub fn attempts(&self) -> u32 {
        self.attempts
    }
    pub fn depth3_tried(&self) -> bool {
        self.depth3_tried
    }
    pub fn depth3_unfloored_tried(&self) -> bool {
        self.depth3_unfloored_tried
    }
    #[cfg(test)]
    pub(crate) fn for_tests(key: Key, depth3_unfloored_tried: bool) -> Self {
        LadderExhausted {
            item: ItemId("it1-test".into()),
            key,
            attempts: 0,
            depth3_tried: true,
            depth3_unfloored_tried,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverMember {
    pub attempt_key: String,
    pub index: usize,
    pub setting: SeedSetting,
    pub digest: String,
    pub rhs_terms: u64,
    /// Gap points this member serves (first member in cover order that credits the key).
    pub served: Vec<Key>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cover {
    pub members: Vec<CoverMember>,
    /// The set cover was computed greedily (more than [`EXACT_COVER_MAX`] candidates).
    pub greedy: bool,
}

impl Cover {
    pub fn cases(&self) -> usize {
        self.members.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// Every gap point is served by the cover at the node's evidence level.
    Covered { cover: Cover },
    /// Part (or none) of the gap points served; the rest assigned to children (sub-cases, then points).
    /// `alternatives`: other standard planes holding left-over points; a driver releases one only for the points its
    /// primary children leave unserved (after their whole subtree), so no point gets two rules.
    /// `reserve`: accepted candidates kept out of the cover by the economy pruning; a driver applies them, in order,
    /// to the points the children's subtrees leave unserved (`served` lists their credited points).
    Split { cover: Cover, children: Vec<Item>, alternatives: Vec<Item>, reserve: Vec<CoverMember> },
    /// A point whose ladder failed: terminal PROPOSAL (L5; declared only at S9, 2.4.9).
    TerminalCandidates { keys: Vec<Key>, exhausted: LadderExhausted },
    /// A counter was exceeded or the input is unusable; never silent.
    Attention { why: String },
    /// Optimise item: no acceptable replacement; the old rules stay (U-13).
    OptimiseKept { why: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Next {
    Run(Vec<Attempt>),
    NeedVerdicts(Vec<VerdictRequest>),
    NeedEvidence(Vec<EvidenceRequest>),
    /// Optimise items only: containment of the old rules' domains in the candidate's.
    NeedContainment(Vec<ContainmentRequest>),
    Done(Disposition),
}

/// State of one item: the item and everything the driver recorded for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemState {
    pub item: Item,
    pub records: BTreeMap<String, AttemptRecord>,
    pub verdicts: BTreeMap<(String, usize), Vec<PointVerdict>>,
    pub evidence: BTreeMap<(String, usize), Vec<KeyEvidence>>,
    pub containment: BTreeMap<(String, usize), Tri>,
}

impl ItemState {
    pub fn new(item: Item) -> Self {
        ItemState {
            item,
            records: BTreeMap::new(),
            verdicts: BTreeMap::new(),
            evidence: BTreeMap::new(),
            containment: BTreeMap::new(),
        }
    }

    pub fn record_containment(&mut self, attempt_key: &str, index: usize, t: Tri) {
        self.containment.insert((attempt_key.to_string(), index), t);
    }

    /// Record the solver's answer to an attempt. An interrupt keeps the key open (counted per kind).
    pub fn record_answer(&mut self, attempt: &Attempt, answer: SolveAnswer) {
        let r = self.records.entry(attempt.key.clone()).or_insert_with(|| AttemptRecord {
            setting: attempt.setting.clone(),
            wave: attempt.wave,
            answer: None,
            operator_interrupts: 0,
            infra_interrupts: 0,
            cpu_ms_total: 0,
        });
        if r.answer.is_some() {
            return; // never re-run, never overwritten
        }
        r.cpu_ms_total += answer.cpu_ms;
        match answer.outcome {
            SolveOutcome::Interrupted {
                by: Interruption::Operator,
            } => r.operator_interrupts += 1,
            SolveOutcome::Interrupted {
                by: Interruption::Infra,
            } => r.infra_interrupts += 1,
            _ => r.answer = Some(answer),
        }
    }

    pub fn record_verdicts(&mut self, attempt_key: &str, index: usize, verdicts: Vec<PointVerdict>) {
        self.verdicts.insert((attempt_key.to_string(), index), verdicts);
    }

    pub fn record_evidence(&mut self, attempt_key: &str, index: usize, ev: Vec<KeyEvidence>) {
        self.evidence.entry((attempt_key.to_string(), index)).or_default().extend(ev);
    }

    /// Attempts with a final answer.
    pub fn attempts_made(&self) -> usize {
        self.records.values().filter(|r| r.is_final()).count()
    }
}

/// Attempt key of an item and setting.
pub fn attempt_key(item: &ItemId, s: &SeedSetting) -> String {
    format!("{}|{}", item.0, s.canonical_key())
}

/// Candidates with more than this many members get a greedy set cover (flagged in [`Cover::greedy`]).
pub const EXACT_COVER_MAX: usize = 24;

/// One accepted candidate: its place, its credited gap points (after verdicts and evidence), its score parts.
#[derive(Clone, Debug)]
pub(crate) struct Accepted {
    pub key: String,
    pub index: usize,
    pub order: usize,
    pub setting: SeedSetting,
    pub rule: CandidateRule,
    /// Gap point indices credited by the verdicts (minus keys whose evidence failed).
    pub credited: BTreeSet<usize>,
    /// Gap point indices still lacking evidence at the required level.
    pub lacking: BTreeSet<usize>,
}

/// The total seed-search space of an item (the sub-counter, D-43).
pub fn sub_counter(state: &ItemState, params: &NodeParams) -> usize {
    seed_space(&state.item.case, params).iter().map(|w| w.len()).sum()
}

fn evidence_ok(ev: &KeyEvidence, min: EvidenceLevel) -> Option<bool> {
    if ev.failure.is_some() {
        return Some(false);
    }
    ev.level.map(|l| l >= min)
}

/// The accepted candidates of the finished attempts in `keys` (in enumeration order), with credited gap points.
/// Returns `Err(requests)` if verdicts are missing.
#[allow(clippy::type_complexity)]
pub(crate) fn accepted_candidates(
    state: &ItemState,
    params: &NodeParams,
    settings: &[(usize, SeedSetting, String)],
) -> Result<Vec<Accepted>, Vec<VerdictRequest>> {
    let gp = &state.item.gap_points;
    let mut need = Vec::new();
    let mut out = Vec::new();
    for (order, setting, key) in settings {
        let Some(rec) = state.records.get(key) else { continue };
        for (idx, cand) in rec.candidates().iter().enumerate() {
            let Some(v) = state.verdicts.get(&(key.clone(), idx)) else {
                need.push(VerdictRequest {
                    attempt_key: key.clone(),
                    index: idx,
                    keys: gp.clone(),
                });
                continue;
            };
            if v.len() != gp.len() || v.iter().any(|x| matches!(x, PointVerdict::HiddenPole)) {
                continue; // incomplete exceptions (or malformed answer): rejected for this item
            }
            let mut credited: BTreeSet<usize> = (0..gp.len()).filter(|&i| v[i].covered()).collect();
            if credited.is_empty() {
                continue; // covers none of its gap points: rejected (the checker found 5 such rules)
            }
            let mut lacking = BTreeSet::new();
            if params.min_evidence > EvidenceLevel::A1 {
                let ev = state.evidence.get(&(key.clone(), idx));
                for &i in &credited.clone() {
                    let e = ev.and_then(|evs| evs.iter().rev().find(|e| e.key == gp[i]));
                    match e.and_then(|e| evidence_ok(e, params.min_evidence)) {
                        Some(true) => {}
                        Some(false) => {
                            credited.remove(&i);
                        }
                        None => {
                            lacking.insert(i);
                        }
                    }
                }
                if credited.is_empty() {
                    continue;
                }
            }
            out.push(Accepted {
                key: key.clone(),
                index: idx,
                order: *order,
                setting: setting.clone(),
                rule: cand.clone(),
                credited,
                lacking,
            });
        }
    }
    if need.is_empty() { Ok(out) } else { Err(need) }
}

fn score(a: &Accepted, n_points: usize) -> SelectionScore {
    SelectionScore {
        failed: (n_points - a.credited.len()) as u64,
        cases: 1,
        rhs_terms: a.rule.rhs_terms,
        monomials: a.rule.monomials.unwrap_or(u64::MAX),
        degree: a.rule.max_degree.unwrap_or(u32::MAX),
        order: a.order as u64,
    }
}

/// Best ONE-CASE cover among accepted candidates (credits every gap point), by the economy selection score.
pub(crate) fn best_one_case(acc: &[Accepted], n_points: usize) -> Option<usize> {
    (0..acc.len())
        .filter(|&i| acc[i].credited.len() == n_points)
        .min_by_key(|&i| score(&acc[i], n_points))
}

/// Minimum set cover over the accepted candidates of the coverable points: exact (fewest members, then smallest
/// RHS volume, then enumeration order) up to [`EXACT_COVER_MAX`] candidates and 6 members, else greedy.
pub(crate) fn min_cover(acc: &[Accepted]) -> (Vec<usize>, bool) {
    let coverable: BTreeSet<usize> = acc.iter().flat_map(|a| a.credited.iter().copied()).collect();
    if coverable.is_empty() {
        return (vec![], false);
    }
    // drop dominated candidates (credited subset of another's, not better): keep the order deterministic
    let n = acc.len();
    if n <= EXACT_COVER_MAX {
        for k in 1..=n.min(6) {
            let mut best: Option<(u64, Vec<usize>)> = None;
            let mut comb: Vec<usize> = (0..k).collect();
            loop {
                let u: BTreeSet<usize> = comb.iter().flat_map(|&i| acc[i].credited.iter().copied()).collect();
                if u == coverable {
                    let vol: u64 = comb.iter().map(|&i| acc[i].rule.rhs_terms).sum();
                    let key: Vec<usize> = comb.iter().map(|&i| acc[i].order * 1000 + acc[i].index).collect();
                    let better = match &best {
                        None => true,
                        Some((bv, bc)) => {
                            let bk: Vec<usize> = bc.iter().map(|&i| acc[i].order * 1000 + acc[i].index).collect();
                            vol < *bv || (vol == *bv && key < bk)
                        }
                    };
                    if better {
                        best = Some((vol, comb.clone()));
                    }
                }
                // next combination
                let mut i = k;
                while i > 0 && comb[i - 1] == n - k + i - 1 {
                    i -= 1;
                }
                if i == 0 {
                    break;
                }
                comb[i - 1] += 1;
                for j in i..k {
                    comb[j] = comb[j - 1] + 1;
                }
            }
            if let Some((_, c)) = best {
                return (c, false);
            }
        }
    }
    // greedy: most new points, then smaller RHS, then order
    let mut left = coverable;
    let mut chosen = Vec::new();
    while !left.is_empty() {
        let pick = (0..n)
            .filter(|i| !chosen.contains(i))
            .max_by(|&a, &b| {
                let ga = acc[a].credited.intersection(&left).count();
                let gb = acc[b].credited.intersection(&left).count();
                ga.cmp(&gb)
                    .then(acc[b].rule.rhs_terms.cmp(&acc[a].rule.rhs_terms))
                    .then(acc[b].order.cmp(&acc[a].order))
            })
            .expect("a candidate covers what is left");
        for p in &acc[pick].credited {
            left.remove(p);
        }
        chosen.push(pick);
    }
    (chosen, true)
}

pub(crate) fn make_cover(state: &ItemState, acc: &[Accepted], members: &[usize], greedy: bool) -> Cover {
    let gp = &state.item.gap_points;
    let mut taken = BTreeSet::new();
    let mut out = Vec::new();
    let mut ms: Vec<usize> = members.to_vec();
    ms.sort_by_key(|&i| (acc[i].order, acc[i].index));
    for i in ms {
        let a = &acc[i];
        let served: Vec<Key> = a
            .credited
            .iter()
            .filter(|p| taken.insert(**p))
            .map(|&p| gp[p].clone())
            .collect();
        out.push(CoverMember {
            attempt_key: a.key.clone(),
            index: a.index,
            setting: a.setting.clone(),
            digest: a.rule.digest.clone(),
            rhs_terms: a.rule.rhs_terms,
            served,
        });
    }
    Cover {
        members: out,
        greedy,
    }
}

/// Request: does candidate (attempt key, index) have a domain CONTAINING the old rules' domains (optimise items,
/// U-13; on FULL nodes a proof by the exact partition, R33)?
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainmentRequest {
    pub attempt_key: String,
    pub index: usize,
    pub old: Vec<String>,
}

/// THE STATE MACHINE (repair-library.md section 4). Pure: same state and params -> same result.
pub fn next_steps(state: &ItemState, params: &NodeParams) -> Next {
    let item = &state.item;
    let gp = &item.gap_points;
    let n_points = gp.len();
    if n_points == 0 && item.kind != ItemKind::Optimise {
        return Next::Done(Disposition::Covered { cover: Cover::default() });
    }
    if item.case.dim() > item.root_dim {
        return Next::Done(Disposition::Attention {
            why: format!("child dimension {} above the root's {}", item.case.dim(), item.root_dim),
        });
    }
    let point = item.kind == ItemKind::Point || item.case.is_point();
    let optimise = item.kind == ItemKind::Optimise;
    let waves = seed_space(&item.case, params);
    let mut done_settings: Vec<(usize, SeedSetting, String)> = Vec::new();
    let mut order = 0usize;
    for (w, wave) in waves.iter().enumerate() {
        let entries: Vec<(usize, SeedSetting, String)> = wave
            .iter()
            .map(|s| {
                order += 1;
                (order, s.clone(), attempt_key(&item.id, s))
            })
            .collect();
        for (_, _, k) in &entries {
            if let Some(r) = state.records.get(k)
                && !r.is_final()
                && r.infra_interrupts > params.budgets.infra_retries
            {
                return Next::Done(Disposition::Attention {
                    why: format!(
                        "attempt {k}: {} infra interrupts (cap {})",
                        r.infra_interrupts, params.budgets.infra_retries
                    ),
                });
            }
        }
        let is_final = |k: &String| state.records.get(k).is_some_and(|r| r.is_final());
        let finished: Vec<(usize, SeedSetting, String)> =
            entries.iter().filter(|e| is_final(&e.2)).cloned().collect();
        let pending: Vec<&(usize, SeedSetting, String)> = entries.iter().filter(|e| !is_final(&e.2)).collect();
        let mut all_done = done_settings.clone();
        all_done.extend(finished.iter().cloned());
        let mut acc = match accepted_candidates(state, params, &all_done) {
            Ok(a) => a,
            Err(need) => return Next::NeedVerdicts(need),
        };
        if optimise {
            match optimise_filter(state, &mut acc) {
                Ok(()) => {}
                Err(req) => return Next::NeedContainment(req),
            }
        }
        let one = match one_case_with_evidence(&acc, gp, params) {
            Ok(o) => o,
            Err(req) => return Next::NeedEvidence(req),
        };
        let offer = match one {
            None => !pending.is_empty(),
            Some(_) if pending.is_empty() => false,
            Some(_) => {
                // economy extra budget (R31 (b), U-13): c0 = CPU of the first acceptable one-case answer in
                // enumeration order; the remaining settings of the wave are offered while the CPU spent on
                // settings after it is <= factor x c0
                let first_ok = acc
                    .iter()
                    .filter(|a| a.credited.len() == n_points && a.lacking.is_empty())
                    .map(|a| a.order)
                    .min()
                    .unwrap_or(0);
                let cpu = |e: &(usize, SeedSetting, String)| state.records.get(&e.2).map_or(0, |r| r.cpu_ms_total);
                let c0 = entries.iter().find(|e| e.0 == first_ok).map_or(0, cpu);
                let spent: u64 = entries.iter().filter(|e| e.0 > first_ok).map(cpu).sum();
                (spent as f64) <= params.budgets.economy_extra_factor * (c0.max(1) as f64)
            }
        };
        if offer {
            let oversized = state.records.values().any(|r| {
                matches!(
                    r.answer,
                    Some(SolveAnswer {
                        outcome: SolveOutcome::RefusedHeavy { .. },
                        ..
                    })
                )
            });
            let attempts: Vec<Attempt> = pending
                .iter()
                .map(|(_, s, k)| {
                    let r = state.records.get(k);
                    Attempt {
                        item: item.id.clone(),
                        key: k.clone(),
                        case: item.case.clone(),
                        setting: s.clone(),
                        rung: if point { Rung::Point } else { Rung::SeedSearch },
                        wave: w as u8,
                        budget: attempt_budget(s, params, point, oversized),
                        seed_args: engine_seed_args(s),
                        attempt_no: 1 + r.map_or(0, |r| r.operator_interrupts + r.infra_interrupts),
                    }
                })
                .collect();
            return Next::Run(attempts);
        }
        if let Some(bi) = one {
            let cover = make_cover(state, &acc, &[bi], false);
            return Next::Done(Disposition::Covered { cover });
        }
        done_settings.extend(finished);
    }
    if optimise {
        return Next::Done(Disposition::OptimiseKept {
            why: "no acceptable replacement with a containing domain and a strictly better economy measure".into(),
        });
    }
    // every wave answered without a one-case cover: minimum set cover, then children / terminal proposal
    let acc = match accepted_candidates(state, params, &done_settings) {
        Ok(a) => a,
        Err(need) => return Next::NeedVerdicts(need),
    };
    let (members, greedy) = min_cover(&acc);
    let mut req = Vec::new();
    for &m in &members {
        let a = &acc[m];
        if !a.lacking.is_empty() {
            req.push(EvidenceRequest {
                attempt_key: a.key.clone(),
                index: a.index,
                keys: a.lacking.iter().map(|&i| gp[i].clone()).collect(),
                level: params.min_evidence,
            });
        }
    }
    if !req.is_empty() {
        return Next::NeedEvidence(req);
    }
    // ECONOMY PRUNING (decision, see repair-library.md 4.2): a member whose own points all lie on exception branches
    // READ FROM an earlier member's rule, where that branch also holds a point the full cover leaves, adds a case
    // the branch child makes anyway: it goes to the RESERVE, applied by the driver only to points the children's
    // subtrees leave unserved (so a child that fails costs no closure, and a child that succeeds saves a case)
    let mut members = members;
    let mut reserve_idx: Vec<usize> = Vec::new();
    loop {
        let full = make_cover(state, &acc, &members, greedy);
        let served_full: BTreeSet<Key> = full.members.iter().flat_map(|m| m.served.iter().cloned()).collect();
        let left_full: Vec<Key> = gp.iter().filter(|k| !served_full.contains(*k)).cloned().collect();
        if left_full.is_empty() || members.len() < 2 {
            break;
        }
        let mut dropped = None;
        for (pos, m) in full.members.iter().enumerate().rev() {
            if pos == 0 || m.served.is_empty() {
                continue;
            }
            let earlier: Vec<&CandidateRule> = full.members[..pos]
                .iter()
                .filter_map(|e| acc.iter().find(|a| a.key == e.attempt_key && a.index == e.index).map(|a| &a.rule))
                .collect();
            let all_on_live_branch = m.served.iter().all(|p| {
                earlier.iter().any(|r| match excepting_branch(r, p) {
                    Some((name, _)) => left_full
                        .iter()
                        .any(|q| excepting_branch(r, q).is_some_and(|(n2, _)| n2 == name)),
                    None => false,
                })
            });
            if all_on_live_branch {
                dropped = acc.iter().position(|a| a.key == m.attempt_key && a.index == m.index);
                break;
            }
        }
        match dropped {
            Some(i) => {
                members.retain(|&x| x != i);
                reserve_idx.push(i);
            }
            None => break,
        }
    }
    let cover = make_cover(state, &acc, &members, greedy);
    let reserve: Vec<CoverMember> = reserve_idx
        .iter()
        .map(|&i| {
            let a = &acc[i];
            CoverMember {
                attempt_key: a.key.clone(),
                index: a.index,
                setting: a.setting.clone(),
                digest: a.rule.digest.clone(),
                rhs_terms: a.rule.rhs_terms,
                served: a.credited.iter().map(|&p| gp[p].clone()).collect(),
            }
        })
        .collect();
    let served: BTreeSet<Key> = cover.members.iter().flat_map(|m| m.served.iter().cloned()).collect();
    let leftover: Vec<Key> = gp.iter().filter(|k| !served.contains(*k)).cloned().collect();
    if leftover.is_empty() {
        return Next::Done(Disposition::Covered { cover });
    }
    if point {
        let tried: Vec<&AttemptRecord> = state.records.values().filter(|r| r.is_final()).collect();
        let exhausted = LadderExhausted {
            item: item.id.clone(),
            key: leftover[0].clone(),
            attempts: tried.len() as u32,
            depth3_tried: tried.iter().any(|r| r.setting.depth >= 3),
            depth3_unfloored_tried: tried.iter().any(|r| r.setting.depth >= 3 && !r.setting.floor),
        };
        return Next::Done(Disposition::TerminalCandidates {
            keys: leftover,
            exhausted,
        });
    }
    let cover_rules: Vec<&CandidateRule> = members.iter().map(|&m| &acc[m].rule).collect();
    // every other judged candidate informs the children, also one that credits no gap point (all of them on its
    // exception branches: plan 2.4.4 "one L2 item per branch whose zero set meets an uncovered target point");
    // candidates with a hidden pole are not trusted for their exception set
    let mut informants: Vec<&CandidateRule> = Vec::new();
    for (_, _, key) in &done_settings {
        let Some(rec) = state.records.get(key) else { continue };
        for (idx, c) in rec.candidates().iter().enumerate() {
            let hidden = state
                .verdicts
                .get(&(key.clone(), idx))
                .is_none_or(|v| v.iter().any(|x| matches!(x, PointVerdict::HiddenPole)));
            if !hidden && !cover_rules.iter().any(|r| r.digest == c.digest) {
                informants.push(c);
            }
        }
    }
    let others = informants;
    let reserve_rules: Vec<&CandidateRule> = reserve_idx.iter().map(|&i| &acc[i].rule).collect();
    let (children, alternatives) = derive_children(item, &cover_rules, &reserve_rules, &others, &leftover, params);
    Next::Done(Disposition::Split {
        cover,
        children,
        alternatives,
        reserve,
    })
}

/// The best one-case candidate whose credited keys all carry evidence at the required level; `Err(request)` when the
/// best one still lacks evidence (a failure there removes keys, and the next candidate becomes the best).
fn one_case_with_evidence(
    acc: &[Accepted],
    gp: &[Key],
    params: &NodeParams,
) -> Result<Option<usize>, Vec<EvidenceRequest>> {
    let Some(bi) = best_one_case(acc, gp.len()) else {
        return Ok(None);
    };
    let a = &acc[bi];
    if a.lacking.is_empty() {
        return Ok(Some(bi));
    }
    Err(vec![EvidenceRequest {
        attempt_key: a.key.clone(),
        index: a.index,
        keys: a.lacking.iter().map(|&i| gp[i].clone()).collect(),
        level: params.min_evidence,
    }])
}

/// Optimise items (U-13): keep only candidates whose domain CONTAINS the old rules' (driver verdict, R33 proof on
/// FULL nodes) and whose measure is STRICTLY better than the old rules' together; ask for missing containment.
fn optimise_filter(state: &ItemState, acc: &mut Vec<Accepted>) -> Result<(), Vec<ContainmentRequest>> {
    let old = &state.item.replaces;
    let old_score = SelectionScore {
        failed: 0,
        cases: old.len() as u64,
        rhs_terms: old.iter().map(|o| o.rhs_terms).sum(),
        monomials: old.iter().map(|o| o.monomials.unwrap_or(u64::MAX / 4)).fold(0u64, |a, b| a.saturating_add(b)),
        degree: old.iter().map(|o| o.max_degree.unwrap_or(u32::MAX)).max().unwrap_or(u32::MAX),
        order: 0,
    };
    let n = state.item.gap_points.len();
    let mut need = Vec::new();
    acc.retain(|a| {
        let s = score(a, n);
        let better = (s.failed, s.cases, s.rhs_terms, s.monomials, s.degree)
            < (old_score.failed, old_score.cases, old_score.rhs_terms, old_score.monomials, old_score.degree);
        if !better || a.credited.len() != n {
            return false;
        }
        match state.containment.get(&(a.key.clone(), a.index)) {
            Some(Tri::Pass) => true,
            Some(_) => false,
            None => {
                need.push(ContainmentRequest {
                    attempt_key: a.key.clone(),
                    index: a.index,
                    old: old.iter().map(|o| o.digest.clone()).collect(),
                });
                false
            }
        }
    });
    if need.is_empty() { Ok(()) } else { Err(need) }
}
