//! The terminal-minimisation STATE MACHINE (interfaces/termmin.md sections 0, 3-5): pure, no process, no file, no
//! clock. The caller asks [`Termmin::pending_solves`] for the recipe solves it may run now (any number at once, any
//! thread count), feeds their answers with [`Termmin::answer`] (by request id, in any order) and calls
//! [`Termmin::advance`] with a [`ClosureOracle`]: every DECISION is made there, one terminal at a time in the fixed
//! order (D+r descending, ties higher under the node comparator first), from answers read as a MAP. Same inputs and the
//! same answers give the same decisions and the same final terminal set, whatever the completion order (determinism),
//! including after an interrupt (a new machine fed the recorded answers replays the same decisions).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::spec::{REPORT_SCHEMA, StepKind, TermminSpec};
use super::steps::{PointRecipe, key_text, point_recipe};

pub type Key = Vec<i32>;

/// Economy numbers of a closure (R48 addendum 4): terminals, closure size, used rules, recipes serving keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Economy {
    pub terminals: u64,
    pub closure_keys: u64,
    pub used_rules: u64,
    pub recipes: u64,
}

/// Measured cost of one solve (never part of a decision).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    pub cpu_s: Option<f64>,
    pub wall_s: Option<f64>,
    pub vmhwm_kb: Option<u64>,
    pub job: Option<String>,
}

/// A candidate relation for a terminal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// `dispatch` | `symmetry` | `recipe`
    pub kind: String,
    pub step: String,
    pub rhs: Vec<Key>,
    pub relation_digest: Option<String>,
    pub recipe_id: Option<String>,
    pub recipe_sha256: Option<String>,
    /// Opaque to the machine, read by the oracle (recipe: `{"solved": <solved-relation v1 line>, "recipe": <doc>}`;
    /// symmetry: `{"coefficients": [[key, "p/q"], ..], "elements": [..]}`).
    pub payload: Value,
}

impl Candidate {
    pub fn dispatch() -> Candidate {
        Candidate {
            kind: "dispatch".into(),
            step: "dispatch".into(),
            rhs: vec![],
            relation_digest: None,
            recipe_id: None,
            recipe_sha256: None,
            payload: Value::Null,
        }
    }
}

/// A solve of the node's own recipe for a key brought by a candidate relation (the closing try).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosingSolve {
    pub source: String,
    pub recipe_path: String,
    pub recipe_sha256: String,
    pub engine_digest: Option<String>,
    pub key: Key,
}

/// What the caller must solve.
#[derive(Clone, Debug, Serialize)]
pub enum SolvePurpose {
    /// A recipe step of the escalation at the terminal itself.
    Step {
        index: usize,
        recipe: Box<PointRecipe>,
    },
    /// A closing-try solve (a node recipe at a key brought by the candidate of step `step`).
    Closing { step: usize, solve: ClosingSolve },
}

/// One solve request (deterministic id).
#[derive(Clone, Debug, Serialize)]
pub struct SolveRequest {
    pub id: String,
    pub terminal: Key,
    pub key: Key,
    pub purpose: SolvePurpose,
}

/// The answer of one solve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SolveAnswer {
    /// A certified relation (accept chain passed); for a closing solve the payload carries its solved line.
    Relation { candidate: Candidate, cost: Cost },
    /// No certified relation: `class` = the verb's failure class (`no_relation_in_seeds`, `budget_wall`, ...) or
    /// `refused` (adapter / accept-chain refusal, lying child) or `resource` (child killed); `line` = its solved-relation
    /// line when one exists (closing solves feed it to the import as an answered request).
    Failed {
        class: String,
        verdict: Option<String>,
        failure: Value,
        line: Option<Value>,
        cost: Cost,
    },
}

/// Result of the symmetry step.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum SymStep {
    Relation(Candidate),
    NoRelation(String),
    Unavailable(String),
}

/// The closure EXTENDED by a candidate (t undeclared, relation added as the last relation set).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extension {
    /// The terminal is served in the extended closure.
    pub served: bool,
    /// Why not (the import's refusal reason / open status).
    pub reason: Option<String>,
    /// Keys open in the extension that are not open in the base.
    pub new_open: Vec<Key>,
    /// Declared terminals reached by the extended closure.
    pub terminals_after: Vec<Key>,
    pub economy: Economy,
}

/// The closure state the decisions are made against (implemented by `termmin::extend::ImportExtension` on an
/// imported set, and by [`KeySetOracle`] on a plain key set).
pub trait ClosureOracle {
    /// `a` strictly lower than `b` under the node comparator.
    fn lower(&self, a: &[i32], b: &[i32]) -> bool;
    fn lines(&self) -> Vec<bool>;
    fn symmetry(&mut self, t: &[i32]) -> SymStep;
    /// The extension check (no state change). `extras` = solved-relation lines of a closing try.
    fn extend(&mut self, t: &[i32], cand: &Candidate, extras: &[Value]) -> Extension;
    /// Solves of the node's recipes for the keys `ext.new_open` (the closing try); empty = none possible.
    fn closing_requests(
        &mut self,
        t: &[i32],
        cand: &Candidate,
        ext: &Extension,
    ) -> Vec<ClosingSolve>;
    /// The candidate is kept: the state now includes it (t no longer terminal).
    fn commit(&mut self, t: &[i32], cand: &Candidate, extras: &[Value]) -> Result<(), String>;
}

/// The extension numbers kept in an attempt record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtRecord {
    pub class: String,
    pub reason: Option<String>,
    pub new_open: Vec<Key>,
    pub before: Economy,
    pub after: Economy,
    pub closing_try: bool,
}

/// One step of a terminal's attempt record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepRecord {
    pub index: usize,
    pub step: String,
    pub kind: String,
    pub skipped: Option<String>,
    pub seeds: Option<u64>,
    pub centres: Vec<Vec<i32>>,
    pub floor: Vec<usize>,
    pub shells: Vec<String>,
    pub symmetry_rows: bool,
    pub relation_rule: Option<String>,
    pub recipe_id: Option<String>,
    pub recipe_sha256: Option<String>,
    /// `relation` | `no_relation` | `refused` | `skipped` | `eliminated`
    pub outcome: String,
    pub verdict: Option<String>,
    pub failure: Value,
    pub rhs_keys: Option<usize>,
    pub extension: Option<ExtRecord>,
    pub cost: Option<Cost>,
}

/// The attempt record of one terminal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerminalRecord {
    pub key: Key,
    pub key_text: String,
    pub d: i64,
    pub r: i64,
    pub order: usize,
    pub preferred: bool,
    /// `eliminated` | `kept` | `preferred` | `no_longer_reached`
    pub outcome: String,
    pub class: String,
    pub steps: Vec<StepRecord>,
    pub eliminated_by: Option<Value>,
}

/// One decision, in decision order (the ledger record of the closing loop; compared on resume).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub seq: usize,
    pub key: Key,
    pub outcome: String,
    pub class: String,
    pub step: Option<String>,
    pub relation_digest: Option<String>,
    pub terminals_after: u64,
    pub closure_after: u64,
}

/// The report (schema `rustred.campaign.termmin-report.v1`).
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub spec_digest: String,
    pub objective: String,
    pub done: bool,
    pub order: Vec<String>,
    pub terminals: Vec<TerminalRecord>,
    pub decisions: Vec<Decision>,
    pub initial_terminals: Vec<Key>,
    pub final_terminals: Vec<Key>,
    pub economy_before: Economy,
    pub economy_after: Economy,
    pub solves_answered: usize,
    pub cpu_s: f64,
    pub kept: Vec<Value>,
}

/// D (total line power) and r (total numerator rank) of a key on a chart.
pub fn d_r(key: &[i32], lines: &[bool]) -> (i64, i64) {
    let mut d = 0i64;
    let mut r = 0i64;
    for (x, l) in key.iter().zip(lines) {
        if *l {
            d += (*x).max(0) as i64;
        } else {
            r += (-*x).max(0) as i64;
        }
    }
    (d, r)
}

/// The attempt order: D+r descending, ties higher under the comparator first (`cmp(a, b) == Less`: a lower).
pub fn attempt_order(
    keys: &[Key],
    lines: &[bool],
    cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
) -> Vec<Key> {
    let mut v = keys.to_vec();
    v.sort_by(|a, b| {
        let (da, ra) = d_r(a, lines);
        let (db, rb) = d_r(b, lines);
        (db + rb).cmp(&(da + ra)).then_with(|| cmp(b, a))
    });
    v.dedup();
    v
}

/// A hand-down key: some chart line at power <= 0 (the key belongs to a subsector, i.e. to another node).
pub fn is_hand_down(key: &[i32], lines: &[bool]) -> bool {
    key.iter().zip(lines).any(|(x, l)| *l && *x <= 0)
}

#[derive(Clone, Debug)]
struct StepState {
    kind: StepKind,
    name: String,
    recipe: Option<PointRecipe>,
    skip: Option<String>,
    record: Option<StepRecord>,
}

#[derive(Clone, Debug)]
struct Closing {
    step: usize,
    ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct TState {
    key: Key,
    d: i64,
    r: i64,
    preferred: bool,
    steps: Vec<StepState>,
    next: usize,
    decided: Option<(String, String)>,
    eliminated_by: Option<Value>,
    closing_tries: u32,
    closing: Option<Closing>,
    /// candidates that failed ONLY with brings_unserved_keys: (step, candidate, closing answers, eliminations count
    /// at the last check), re-checked once after every later elimination
    retry: Vec<(usize, Candidate, Vec<Value>, usize)>,
    last_relation_class: Option<String>,
    refused: bool,
}

/// The machine.
pub struct Termmin {
    spec: TermminSpec,
    lines: Vec<bool>,
    ts: Vec<TState>,
    answers: BTreeMap<String, SolveAnswer>,
    current: Economy,
    base: Economy,
    current_terminals: BTreeSet<Key>,
    initial: Vec<Key>,
    decisions: Vec<Decision>,
    closing_reqs: BTreeMap<String, (usize, ClosingSolve)>,
}

fn rec_base(i: usize, st: &StepState) -> StepRecord {
    let r = st.recipe.as_ref();
    StepRecord {
        index: i,
        step: st.name.clone(),
        kind: st.kind.word().into(),
        skipped: st.skip.clone(),
        seeds: r.map(|r| r.seeds),
        centres: r.map(|r| r.centres.clone()).unwrap_or_default(),
        floor: r.map(|r| r.floor.clone()).unwrap_or_default(),
        shells: r.map(|r| r.shells.clone()).unwrap_or_default(),
        symmetry_rows: r.is_some_and(|r| r.symmetry_rows),
        relation_rule: r.map(|r| r.relation_rule.clone()),
        recipe_id: r.map(|r| r.id.clone()),
        recipe_sha256: r.map(|r| r.sha256.clone()),
        outcome: if st.skip.is_some() {
            "skipped"
        } else {
            "pending"
        }
        .into(),
        verdict: None,
        failure: Value::Null,
        rhs_keys: None,
        extension: None,
        cost: None,
    }
}

enum Eval {
    Eliminated(Extension, Vec<Value>),
    Failed(ExtRecord),
    Waiting,
}

impl Termmin {
    /// `terminals` = the declared terminals reached by the closed closure; `preferred` = never attempted.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        spec: &TermminSpec,
        terminals: &[Key],
        preferred: &BTreeSet<Key>,
        lines: &[bool],
        line_priority: &[usize],
        num_priority: &[usize],
        base: Economy,
        cmp: &dyn Fn(&[i32], &[i32]) -> Ordering,
    ) -> Result<Termmin, String> {
        spec.validate()?;
        let kinds = spec.steps()?;
        let order = attempt_order(terminals, lines, cmp);
        let mut ts = vec![];
        for key in &order {
            if key.len() != lines.len() {
                return Err(format!(
                    "terminal {key:?} of arity {} for a chart of {}",
                    key.len(),
                    lines.len()
                ));
            }
            let (d, r) = d_r(key, lines);
            let pref = preferred.contains(key);
            let mut steps = vec![];
            let mut seen: Vec<(String, String)> = vec![];
            let (mut used_steps, mut used_seeds) = (0usize, 0u64);
            for k in &kinds {
                let (name, recipe) = match k {
                    StepKind::Dispatch => ("dispatch".to_string(), None),
                    StepKind::SymmetryRelation => ("symmetry_relation".to_string(), None),
                    StepKind::Recipe(s) => (
                        s.name.clone(),
                        Some(point_recipe(s, key, lines, line_priority, num_priority)?),
                    ),
                };
                let mut skip = None;
                if let Some(r) = &recipe {
                    if r.seeds > spec.caps.max_seeds {
                        skip = Some(format!(
                            "over_seed_cap ({} > {})",
                            r.seeds, spec.caps.max_seeds
                        ));
                    } else if let Some((n, _)) = seen.iter().find(|(_, d)| *d == r.seed_set_digest)
                    {
                        skip = Some(format!("same_seed_set_as:{n}"));
                    }
                }
                if skip.is_none() {
                    let seeds = recipe.as_ref().map(|r| r.seeds).unwrap_or(0);
                    if used_steps + 1 > spec.budget.max_steps_per_terminal {
                        skip = Some("budget (max_steps_per_terminal)".into());
                    } else if used_seeds + seeds > spec.budget.max_seeds_per_terminal {
                        skip = Some("budget (max_seeds_per_terminal)".into());
                    } else {
                        used_steps += 1;
                        used_seeds += seeds;
                        if let Some(r) = &recipe {
                            seen.push((name.clone(), r.seed_set_digest.clone()));
                        }
                    }
                }
                steps.push(StepState {
                    kind: k.clone(),
                    name,
                    recipe,
                    skip,
                    record: None,
                });
            }
            ts.push(TState {
                key: key.clone(),
                d,
                r,
                preferred: pref,
                steps,
                next: 0,
                decided: if pref {
                    Some(("preferred".into(), "preferred_not_attempted".into()))
                } else {
                    None
                },
                eliminated_by: None,
                closing_tries: 0,
                closing: None,
                retry: vec![],
                last_relation_class: None,
                refused: false,
            });
        }
        Ok(Termmin {
            spec: spec.clone(),
            lines: lines.to_vec(),
            ts,
            answers: BTreeMap::new(),
            current: base,
            base,
            current_terminals: order.iter().cloned().collect(),
            initial: order,
            decisions: vec![],
            closing_reqs: BTreeMap::new(),
        })
    }

    /// The attempt order (all terminals reached, preferred ones included and marked).
    pub fn order(&self) -> Vec<Key> {
        self.ts.iter().map(|t| t.key.clone()).collect()
    }

    fn step_id(i: usize, s: usize) -> String {
        format!("t{i}.s{s}")
    }

    /// The solves wanted now: for every undecided terminal its first unanswered recipe step (speculatively ahead of
    /// the decision cursor, never past a step whose answer is a relation), and every closing-try solve not answered.
    pub fn pending_solves(&self) -> Vec<SolveRequest> {
        let mut out = vec![];
        for (i, t) in self.ts.iter().enumerate() {
            if t.decided.is_some() {
                continue;
            }
            if let Some(c) = &t.closing {
                for id in &c.ids {
                    if !self.answers.contains_key(id)
                        && let Some((_, cs)) = self.closing_reqs.get(id)
                    {
                        out.push(SolveRequest {
                            id: id.clone(),
                            terminal: t.key.clone(),
                            key: cs.key.clone(),
                            purpose: SolvePurpose::Closing {
                                step: c.step,
                                solve: cs.clone(),
                            },
                        });
                    }
                }
                continue;
            }
            for (s, st) in t.steps.iter().enumerate().skip(t.next) {
                if st.skip.is_some() {
                    continue;
                }
                let Some(r) = &st.recipe else { continue };
                let id = Self::step_id(i, s);
                match self.answers.get(&id) {
                    None => {
                        out.push(SolveRequest {
                            id,
                            terminal: t.key.clone(),
                            key: t.key.clone(),
                            purpose: SolvePurpose::Step {
                                index: s,
                                recipe: Box::new(r.clone()),
                            },
                        });
                        break;
                    }
                    Some(SolveAnswer::Relation { .. }) => break,
                    Some(SolveAnswer::Failed { .. }) => continue,
                }
            }
        }
        out
    }

    /// Record the answer of a request (a second answer for the same id is refused).
    pub fn answer(&mut self, id: &str, a: SolveAnswer) -> Result<(), String> {
        // a closing-try id (`t<i>.c<step>.<try>.<j>`) may be answered before this machine generated it (resume: the
        // recorded answers are fed first, the closing try is re-derived by `advance`)
        let closing_shape = id
            .strip_prefix('t')
            .and_then(|x| x.split_once(".c"))
            .is_some_and(|(i, rest)| {
                i.parse::<usize>().is_ok_and(|i| i < self.ts.len())
                    && rest.split('.').count() == 3
                    && rest.split('.').all(|p| p.parse::<usize>().is_ok())
            });
        let known = closing_shape
            || self.closing_reqs.contains_key(id)
            || id
                .strip_prefix('t')
                .and_then(|x| x.split_once(".s"))
                .and_then(|(i, s)| Some((i.parse::<usize>().ok()?, s.parse::<usize>().ok()?)))
                .is_some_and(|(i, s)| {
                    self.ts
                        .get(i)
                        .and_then(|t| t.steps.get(s))
                        .is_some_and(|st| st.recipe.is_some())
                });
        if !known {
            return Err(format!("termmin: answer for unknown request {id}"));
        }
        if let Some(old) = self.answers.get(id) {
            return if *old == a {
                Ok(())
            } else {
                Err(format!("termmin: a second, different answer for {id}"))
            };
        }
        self.answers.insert(id.to_string(), a);
        Ok(())
    }

    /// Answered request ids (for resume bookkeeping).
    pub fn answered(&self) -> usize {
        self.answers.len()
    }

    pub fn done(&self) -> bool {
        self.ts.iter().all(|t| t.decided.is_some())
    }

    pub fn decisions(&self) -> &[Decision] {
        &self.decisions
    }

    pub fn current_terminals(&self) -> Vec<Key> {
        self.current_terminals.iter().cloned().collect()
    }

    pub fn economy(&self) -> Economy {
        self.current
    }

    fn decide(
        &mut self,
        i: usize,
        outcome: &str,
        class: &str,
        step: Option<String>,
        digest: Option<String>,
    ) {
        self.ts[i].decided = Some((outcome.into(), class.into()));
        let seq = self.decisions.len();
        self.decisions.push(Decision {
            seq,
            key: self.ts[i].key.clone(),
            outcome: outcome.into(),
            class: class.into(),
            step,
            relation_digest: digest,
            terminals_after: self.current.terminals,
            closure_after: self.current.closure_keys,
        });
    }

    /// Make every decision the answers allow, in order.
    pub fn advance(&mut self, oracle: &mut dyn ClosureOracle) -> Result<(), String> {
        loop {
            let Some(i) = self.ts.iter().position(|t| t.decided.is_none()) else {
                return Ok(());
            };
            if !self.current_terminals.contains(&self.ts[i].key) {
                self.decide(i, "no_longer_reached", "no_longer_reached", None, None);
                continue;
            }
            if !self.step_terminal(i, oracle)? {
                return Ok(());
            }
        }
    }

    fn set_record(&mut self, i: usize, s: usize, f: impl FnOnce(&mut StepRecord)) {
        let st = &self.ts[i].steps[s];
        let mut r = st.record.clone().unwrap_or_else(|| rec_base(s, st));
        f(&mut r);
        self.ts[i].steps[s].record = Some(r);
    }

    /// Process terminal `i` from its current step; true = decided.
    fn step_terminal(&mut self, i: usize, oracle: &mut dyn ClosureOracle) -> Result<bool, String> {
        let t = self.ts[i].key.clone();
        while self.ts[i].next < self.ts[i].steps.len() {
            let s = self.ts[i].next;
            if self.ts[i].steps[s].skip.is_some() {
                self.set_record(i, s, |_| {});
                self.ts[i].next += 1;
                continue;
            }
            let cand: Candidate = match self.ts[i].steps[s].kind.clone() {
                StepKind::Dispatch => Candidate::dispatch(),
                StepKind::SymmetryRelation => match oracle.symmetry(&t) {
                    SymStep::Relation(c) => c,
                    SymStep::NoRelation(w) => {
                        self.set_record(i, s, |r| {
                            r.outcome = "no_relation".into();
                            r.failure = json!(w);
                        });
                        self.ts[i].next += 1;
                        continue;
                    }
                    SymStep::Unavailable(w) => {
                        self.set_record(i, s, |r| {
                            r.outcome = "skipped".into();
                            r.skipped = Some(w);
                        });
                        self.ts[i].next += 1;
                        continue;
                    }
                },
                StepKind::Recipe(_) => match self.answers.get(&Self::step_id(i, s)).cloned() {
                    None => return Ok(false),
                    Some(SolveAnswer::Failed {
                        class,
                        verdict,
                        failure,
                        cost,
                        ..
                    }) => {
                        let refused = class != "no_relation_in_seeds";
                        if refused {
                            self.ts[i].refused = true;
                        }
                        self.set_record(i, s, |r| {
                            r.outcome = if refused { "refused" } else { "no_relation" }.into();
                            r.verdict = verdict.or(Some(class.to_uppercase()));
                            r.failure = failure;
                            r.cost = Some(cost);
                        });
                        self.ts[i].next += 1;
                        continue;
                    }
                    Some(SolveAnswer::Relation { candidate, cost }) => {
                        self.set_record(i, s, |r| {
                            r.cost = Some(cost);
                            r.verdict = Some("SOLVED".into());
                        });
                        candidate
                    }
                },
            };
            match self.evaluate(i, s, &cand, oracle)? {
                Eval::Waiting => return Ok(false),
                Eval::Eliminated(ext, extras) => {
                    oracle.commit(&t, &cand, &extras)?;
                    self.eliminate(i, s, &cand, ext, false);
                    self.reevaluate(oracle)?;
                    return Ok(true);
                }
                Eval::Failed(er) => {
                    let dispatch_miss =
                        cand.kind == "dispatch" && er.class == "relation_refused_by_import";
                    if !dispatch_miss {
                        self.ts[i].last_relation_class = Some(er.class.clone());
                    }
                    self.set_record(i, s, |r| {
                        r.outcome = if dispatch_miss {
                            "no_relation"
                        } else {
                            "relation"
                        }
                        .into();
                        r.rhs_keys = Some(cand.rhs.len());
                        r.extension = Some(er);
                    });
                    self.ts[i].next += 1;
                }
            }
        }
        // every step done: kept
        let any_budget = self.ts[i]
            .steps
            .iter()
            .any(|s| s.skip.as_deref().is_some_and(|w| w.starts_with("budget")));
        let class = match (
            &self.ts[i].last_relation_class,
            self.ts[i].refused,
            any_budget,
        ) {
            (Some(c), _, _) => c.clone(),
            (None, true, _) => "refused".into(),
            (None, false, true) => "budget".into(),
            _ => "no_relation_in_seeds".into(),
        };
        self.decide(i, "kept", &class, None, None);
        Ok(true)
    }

    fn evaluate(
        &mut self,
        i: usize,
        s: usize,
        cand: &Candidate,
        oracle: &mut dyn ClosureOracle,
    ) -> Result<Eval, String> {
        let t = self.ts[i].key.clone();
        let before = self.current;
        let fail = |class: &str,
                    reason: Option<String>,
                    new_open: Vec<Key>,
                    after: Economy,
                    closing: bool| {
            Eval::Failed(ExtRecord {
                class: class.into(),
                reason,
                new_open,
                before,
                after,
                closing_try: closing,
            })
        };
        // 1. local descent (the brief's definition; a lying child is caught here or by the import)
        if cand.kind != "dispatch" {
            for k in &cand.rhs {
                let bad = if k.len() != t.len() {
                    Some("arity")
                } else if *k == t {
                    Some("the terminal itself on the right-hand side")
                } else if k.iter().zip(&self.lines).any(|(x, l)| !*l && *x > 0) {
                    Some("positive numerator")
                } else if !is_hand_down(k, &self.lines) && !oracle.lower(k, &t) {
                    Some("not lower under the node comparator")
                } else {
                    None
                };
                if let Some(w) = bad {
                    return Ok(fail(
                        "relation_does_not_descend",
                        Some(format!("{}: {w}", key_text(k))),
                        vec![],
                        before,
                        false,
                    ));
                }
            }
        }
        // 2. the extension (with the closing try's answers when they are all in)
        let mut extras: Vec<Value> = vec![];
        let mut closing_done = false;
        if let Some(c) = self.ts[i].closing.clone()
            && c.step == s
        {
            if c.ids.iter().any(|id| !self.answers.contains_key(id)) {
                return Ok(Eval::Waiting);
            }
            for id in &c.ids {
                match &self.answers[id] {
                    SolveAnswer::Relation { candidate, .. } => {
                        if let Some(l) = candidate.payload.get("solved") {
                            extras.push(l.clone());
                        }
                    }
                    SolveAnswer::Failed { line: Some(l), .. } => extras.push(l.clone()),
                    SolveAnswer::Failed { .. } => {}
                }
            }
            closing_done = true;
            self.ts[i].closing = None;
        }
        let ext = oracle.extend(&t, cand, &extras);
        if !ext.served {
            return Ok(fail(
                "relation_refused_by_import",
                ext.reason.clone(),
                ext.new_open.clone(),
                ext.economy,
                closing_done,
            ));
        }
        if !ext.new_open.is_empty() {
            let b = &self.spec.budget;
            if !closing_done
                && self.ts[i].closing_tries < b.max_closing_tries_per_terminal
                && ext.new_open.len() <= b.max_closing_keys
            {
                let reqs = oracle.closing_requests(&t, cand, &ext);
                if !reqs.is_empty() {
                    let try_no = self.ts[i].closing_tries;
                    self.ts[i].closing_tries += 1;
                    let mut ids = vec![];
                    for (j, cs) in reqs.into_iter().enumerate() {
                        let id = format!("t{i}.c{s}.{try_no}.{j}");
                        self.closing_reqs.insert(id.clone(), (s, cs));
                        ids.push(id);
                    }
                    self.ts[i].closing = Some(Closing { step: s, ids });
                    return Ok(Eval::Waiting);
                }
            }
            let n = self.eliminations();
            self.ts[i].retry.push((s, cand.clone(), extras, n));
            return Ok(fail(
                "brings_unserved_keys",
                Some(format!("{} key(s) left open", ext.new_open.len())),
                ext.new_open.clone(),
                ext.economy,
                closing_done,
            ));
        }
        // 3. objective: (terminals, closure size) strictly smaller, lexicographically
        let after = ext.economy;
        if (after.terminals, after.closure_keys) < (before.terminals, before.closure_keys) {
            Ok(Eval::Eliminated(ext, extras))
        } else {
            Ok(fail("no_gain", None, vec![], after, closing_done))
        }
    }

    fn eliminations(&self) -> usize {
        self.decisions
            .iter()
            .filter(|d| d.outcome == "eliminated")
            .count()
    }

    fn eliminate(
        &mut self,
        i: usize,
        s: usize,
        cand: &Candidate,
        ext: Extension,
        reevaluated: bool,
    ) {
        let before = self.current;
        self.current = ext.economy;
        self.current_terminals = ext.terminals_after.iter().cloned().collect();
        self.set_record(i, s, |r| {
            r.outcome = "eliminated".into();
            r.rhs_keys = Some(cand.rhs.len());
            r.extension = Some(ExtRecord {
                class: if reevaluated {
                    "eliminated_on_reevaluation"
                } else {
                    "eliminated"
                }
                .into(),
                reason: None,
                new_open: vec![],
                before,
                after: ext.economy,
                closing_try: false,
            });
        });
        self.ts[i].eliminated_by = Some(json!({"step": cand.step, "kind": cand.kind,
            "recipe_id": cand.recipe_id, "recipe_sha256": cand.recipe_sha256,
            "relation_digest": cand.relation_digest, "rhs_keys": cand.rhs.len(),
            "rhs": cand.rhs.iter().map(|k| key_text(k)).collect::<Vec<_>>(),
            "on_reevaluation": reevaluated}));
        self.ts[i].retry.clear();
        self.ts[i].closing = None;
        self.decide(
            i,
            "eliminated",
            if reevaluated {
                "eliminated_on_reevaluation"
            } else {
                "eliminated"
            },
            Some(cand.step.clone()),
            cand.relation_digest.clone(),
        );
    }

    /// After an elimination: a decided-KEPT terminal whose cached relation failed only with brings_unserved_keys is
    /// re-checked once against the new closure (no new solve), in the attempt order, until nothing changes.
    fn reevaluate(&mut self, oracle: &mut dyn ClosureOracle) -> Result<(), String> {
        loop {
            let n = self.eliminations();
            let mut changed = false;
            for i in 0..self.ts.len() {
                let kept = matches!(&self.ts[i].decided, Some((o, _)) if o == "kept");
                if !kept || !self.current_terminals.contains(&self.ts[i].key) {
                    continue;
                }
                let t = self.ts[i].key.clone();
                let cands: Vec<(usize, Candidate, Vec<Value>)> = self.ts[i]
                    .retry
                    .iter()
                    .filter(|(_, _, _, at)| *at < n)
                    .map(|(s, c, e, _)| (*s, c.clone(), e.clone()))
                    .collect();
                for r in self.ts[i].retry.iter_mut() {
                    r.3 = n;
                }
                for (s, c, extras) in cands {
                    let ext = oracle.extend(&t, &c, &extras);
                    let before = self.current;
                    if ext.served
                        && ext.new_open.is_empty()
                        && (ext.economy.terminals, ext.economy.closure_keys)
                            < (before.terminals, before.closure_keys)
                    {
                        oracle.commit(&t, &c, &extras)?;
                        self.eliminate(i, s, &c, ext, true);
                        changed = true;
                        break;
                    }
                }
                if changed {
                    break;
                }
            }
            if !changed {
                return Ok(());
            }
        }
    }

    /// The report (deterministic content except the `cost` blocks).
    pub fn report(&self, spec_digest: &str) -> Report {
        let mut cpu = 0.0;
        let mut terminals = vec![];
        let mut kept = vec![];
        for (i, t) in self.ts.iter().enumerate() {
            let steps: Vec<StepRecord> = t
                .steps
                .iter()
                .enumerate()
                .map(|(s, st)| st.record.clone().unwrap_or_else(|| rec_base(s, st)))
                .collect();
            for s in &steps {
                if let Some(c) = &s.cost {
                    cpu += c.cpu_s.unwrap_or(0.0);
                }
            }
            let (outcome, class) = t
                .decided
                .clone()
                .unwrap_or_else(|| ("undecided".into(), "undecided".into()));
            if outcome == "kept" || outcome == "preferred" {
                kept.push(json!({"key": key_text(&t.key), "class": class}));
            }
            terminals.push(TerminalRecord {
                key: t.key.clone(),
                key_text: key_text(&t.key),
                d: t.d,
                r: t.r,
                order: i,
                preferred: t.preferred,
                outcome,
                class,
                steps,
                eliminated_by: t.eliminated_by.clone(),
            });
        }
        Report {
            schema: REPORT_SCHEMA,
            spec_digest: spec_digest.to_string(),
            objective: self.spec.objective.clone(),
            done: self.done(),
            order: self.ts.iter().map(|t| key_text(&t.key)).collect(),
            terminals,
            decisions: self.decisions.clone(),
            initial_terminals: self.initial.clone(),
            final_terminals: self.current_terminals(),
            economy_before: self.base,
            economy_after: self.current,
            solves_answered: self.answers.len(),
            cpu_s: cpu,
            kept,
        }
    }
}

/// A boxed key comparator (`Less` = the first key is lower).
pub type KeyCmp = Box<dyn Fn(&[i32], &[i32]) -> Ordering>;

/// A [`ClosureOracle`] on a plain closed KEY SET (tests, and keys-level checks): `served` = closure keys served,
/// `terminals` = declared terminals reached; a candidate keeps the closure closed iff every right-hand-side key is served,
/// a remaining terminal or a hand-down; otherwise the other keys are NEW OPEN keys, which `closing` (key -> right-hand
/// side keys of a closing relation, applied when the closing answers name the key) may serve.
pub struct KeySetOracle {
    pub lines: Vec<bool>,
    pub served: BTreeSet<Key>,
    pub terminals: BTreeSet<Key>,
    pub used_rules: u64,
    /// `cmp(a, b) == Less` iff a lower.
    pub cmp: KeyCmp,
    /// Symmetry answers by key (tests).
    pub symmetry: BTreeMap<Key, Candidate>,
    /// Closing-try solves per new key (tests): the source to ask.
    pub closing_source: Option<String>,
    pub commits: Vec<Key>,
}

impl KeySetOracle {
    fn extended(
        &self,
        t: &[i32],
        cand: &Candidate,
        extras: &[Value],
    ) -> (Extension, BTreeSet<Key>) {
        let mut served = self.served.clone();
        let mut terms = self.terminals.clone();
        terms.remove(t);
        // closing answers: solved lines {"key": [..], "verdict": "SOLVED", "rhs": [{"key": [..]}]}
        let mut closing: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
        for l in extras {
            if l["verdict"] == "SOLVED"
                && let Ok(k) = serde_json::from_value::<Key>(l["key"].clone())
            {
                let rhs: Vec<Key> = l["rhs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| serde_json::from_value(x["key"].clone()).ok())
                    .collect();
                closing.insert(k, rhs);
            }
        }
        if cand.kind == "dispatch" {
            return (
                Extension {
                    served: false,
                    reason: Some("no rule of the dispatch serves the key".into()),
                    ..Extension::default()
                },
                served,
            );
        }
        let ok = |k: &Key, served: &BTreeSet<Key>, terms: &BTreeSet<Key>| {
            served.contains(k) || terms.contains(k) || is_hand_down(k, &self.lines)
        };
        let mut open: BTreeSet<Key> = BTreeSet::new();
        let mut stack: Vec<Key> = cand.rhs.clone();
        let mut added: BTreeSet<Key> = BTreeSet::new();
        while let Some(k) = stack.pop() {
            if ok(&k, &served, &terms) || added.contains(&k) || open.contains(&k) {
                continue;
            }
            match closing.get(&k) {
                Some(rhs) => {
                    added.insert(k.clone());
                    stack.extend(rhs.iter().cloned());
                }
                None => {
                    open.insert(k);
                }
            }
        }
        served.extend(added.iter().cloned());
        served.insert(t.to_vec());
        let n_closure = (served.len() + terms.len()) as u64;
        (
            Extension {
                served: true,
                reason: None,
                new_open: open.into_iter().collect(),
                terminals_after: terms.iter().cloned().collect(),
                economy: Economy {
                    terminals: terms.len() as u64,
                    closure_keys: n_closure,
                    used_rules: self.used_rules + 1 + added.len() as u64,
                    recipes: 0,
                },
            },
            served,
        )
    }
}

impl ClosureOracle for KeySetOracle {
    fn lower(&self, a: &[i32], b: &[i32]) -> bool {
        (self.cmp)(a, b) == Ordering::Less
    }
    fn lines(&self) -> Vec<bool> {
        self.lines.clone()
    }
    fn symmetry(&mut self, t: &[i32]) -> SymStep {
        match self.symmetry.get(t) {
            Some(c) => SymStep::Relation(c.clone()),
            None => SymStep::Unavailable("no verified group".into()),
        }
    }
    fn extend(&mut self, t: &[i32], cand: &Candidate, extras: &[Value]) -> Extension {
        self.extended(t, cand, extras).0
    }
    fn closing_requests(
        &mut self,
        _t: &[i32],
        _cand: &Candidate,
        ext: &Extension,
    ) -> Vec<ClosingSolve> {
        match &self.closing_source {
            None => vec![],
            Some(src) => ext
                .new_open
                .iter()
                .map(|k| ClosingSolve {
                    source: src.clone(),
                    recipe_path: String::new(),
                    recipe_sha256: String::new(),
                    engine_digest: None,
                    key: k.clone(),
                })
                .collect(),
        }
    }
    fn commit(&mut self, t: &[i32], cand: &Candidate, extras: &[Value]) -> Result<(), String> {
        let (ext, served) = self.extended(t, cand, extras);
        if !ext.served || !ext.new_open.is_empty() {
            return Err("commit of a candidate that does not keep the closure closed".into());
        }
        self.served = served;
        self.terminals.remove(t);
        self.used_rules = ext.economy.used_rules;
        self.commits.push(t.to_vec());
        Ok(())
    }
}
