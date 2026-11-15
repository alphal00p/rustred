//! PER-KEY minimisation (interfaces/seedmin.md 4): start solve, support step, greedy drops in the policy's order with
//! the acceptance of [`super::accept`], optional re-support after each kept drop, work budget `max_solves`. A state
//! machine (one batch of trials at a time) so that several keys can be minimised in lockstep (joint step 2); the
//! decisions depend only on the answers, in a fixed order.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::accept::{State, judge_change};
use super::order::{drop_order, restrict, row_ordered, without};
use super::policy::SeedMinPolicy;
use super::{
    Key, Offset, RelationFacts, SeedMinError, SystemPoles, Trial, TrialAnswer, TrialMode,
    TrialSolver, key_label, list_digest, run_batch, sub,
};

/// One trial as recorded (the emulation's trial record; cost data stays with the solver).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialLog {
    pub label: String,
    pub digest: String,
    /// start | support | drop | resupport | confirm | screen | verify | choice | probe_start | probe_drop | probe_exact
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<Offset>,
    pub keys: usize,
    pub seeds: usize,
    /// Per-key answer words (compact).
    pub answers: Vec<String>,
    /// The decision taken from this trial (None: no decision, e.g. a speculative trial after the first success).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<bool>,
    pub reason: String,
    /// Counted in the sequential solve count (`minimise.solves`).
    pub counted: bool,
}

/// The result of a per-key minimisation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyMin {
    pub key: Key,
    pub centre: Offset,
    pub backend: String,
    /// The row order of every list (TRICKS 17 (2): sizes are stated with it).
    pub row_order: String,
    /// The minimised list (centre-relative, list order); the start list when nothing was dropped.
    pub list: Vec<Offset>,
    /// The final relation (None when the start list did not solve).
    pub relation: Option<RelationFacts>,
    pub start_relation: Option<RelationFacts>,
    pub seeds_before: usize,
    /// done | max_solves | no_start | undecided_start
    pub stopped: String,
    /// applied | refused: <reason> | unavailable | not_smaller | off
    pub support: String,
    /// The contract's line-7 `minimise` member (None without a start relation or with mode `off`).
    pub minimise: Option<Value>,
    /// Sequential re-solves (= `minimise.solves`).
    pub solves: u64,
    /// Trials actually run (incl. the start and speculative ones).
    pub trials_run: u64,
    pub trials: Vec<TrialLog>,
    /// TRICKS 14 (c) diagnostic pass (None when `rows_at_d0` is off).
    #[serde(default)]
    pub rows_at_d0: Option<Value>,
}

impl KeyMin {
    /// The backend-independent RESULT (interchangeability: the emulation and the engine verb give the same):
    /// key, centre, list (+ sha256), relation digest, terms, pole order, the minimise member without `seeds`.
    pub fn result_record(&self) -> Value {
        let mut m = self.minimise.clone().unwrap_or(Value::Null);
        if let Some(o) = m.as_object_mut() {
            o.remove("seeds");
        }
        json!({"key": self.key, "centre": self.centre, "row_order": self.row_order, "list": self.list,
            "list_sha256": list_digest(&self.list),
            "relation_digest": self.relation.as_ref().map(|r| r.relation_digest.clone()),
            "rhs_count": self.relation.as_ref().map(|r| r.rhs_count),
            "pole": self.relation.as_ref().map(|r| r.pole),
            "stopped": self.stopped, "minimise": m})
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Start,
    Support,
    Greedy,
    Resupport,
    Done,
}

struct Pending {
    kind: &'static str,
    removed: Option<Offset>,
    list: Vec<Offset>,
    trial: Trial,
}

/// The per-key state machine.
pub(crate) struct KeyMachine<'p> {
    policy: &'p SeedMinPolicy,
    label: String,
    phase: Phase,
    order: Option<Vec<Offset>>,
    tried: BTreeSet<Offset>,
    pending: Vec<Pending>,
    seq: usize,
    out: KeyMin,
}

impl<'p> KeyMachine<'p> {
    /// A machine at `key`; `start` = the answer of the start list at the key when the caller already has it (joint
    /// step 1: the same list at the same key gives the same relation).
    pub(crate) fn new(
        key: &[i32],
        centre: &[i32],
        list: &[Offset],
        policy: &'p SeedMinPolicy,
        label: String,
        start: Option<&TrialAnswer>,
    ) -> KeyMachine<'p> {
        let mut m = KeyMachine {
            policy,
            label,
            phase: Phase::Start,
            order: None,
            tried: BTreeSet::new(),
            pending: vec![],
            seq: 0,
            out: KeyMin {
                key: key.to_vec(),
                centre: centre.to_vec(),
                backend: "emulated".into(),
                row_order: policy.row_order.clone(),
                list: list.to_vec(),
                relation: None,
                start_relation: None,
                seeds_before: list.len(),
                stopped: String::new(),
                support: "off".into(),
                minimise: None,
                solves: 0,
                trials_run: 0,
                trials: vec![],
                rows_at_d0: None,
            },
        };
        if let Some(a) = start {
            m.take_start(a.clone(), None);
        }
        m
    }

    fn trial(&mut self, kind: &'static str, list: Vec<Offset>, removed: Option<Offset>) -> Pending {
        self.seq += 1;
        let t = Trial {
            label: format!("{}/{}/{}", self.label, self.seq, kind),
            centre: self.out.centre.clone(),
            list: list.clone(),
            keys: vec![self.out.key.clone()],
            mode: TrialMode::Solve,
            engine_minimise: None,
        };
        Pending {
            kind,
            removed,
            list,
            trial: t,
        }
    }

    fn log(
        &mut self,
        p: &Pending,
        a: &TrialAnswer,
        kept: Option<bool>,
        reason: String,
        counted: bool,
    ) {
        self.out.trials.push(TrialLog {
            label: p.trial.label.clone(),
            digest: p.trial.digest(),
            kind: p.kind.into(),
            removed: p.removed.clone(),
            keys: 1,
            seeds: p.list.len(),
            answers: vec![a.word()],
            kept,
            reason,
            counted,
        });
    }

    fn take_start(&mut self, a: TrialAnswer, p: Option<&Pending>) {
        if let Some(p) = p {
            self.log(p, &a, None, "start".into(), false);
        }
        match a {
            TrialAnswer::Relation(r) => {
                self.out.start_relation = Some((*r).clone());
                self.out.relation = Some(*r);
                self.phase = Phase::Support;
            }
            TrialAnswer::NoRelation | TrialAnswer::NotInDomain => {
                self.out.stopped = "no_start".into();
                self.phase = Phase::Done;
            }
            _ => {
                self.out.stopped = "undecided_start".into();
                self.phase = Phase::Done;
            }
        }
    }

    fn budget_left(&self) -> u64 {
        self.policy.max_solves.saturating_sub(self.out.solves)
    }

    fn support_list(&self) -> Option<Vec<Offset>> {
        let used = self.out.relation.as_ref()?.seeds_used.as_ref()?;
        let r = restrict(&self.out.list, used);
        (!r.is_empty() && r.len() < self.out.list.len()).then_some(r)
    }

    /// The next batch (empty = done). Phases without a trial are passed through.
    pub(crate) fn next_batch(&mut self) -> Vec<Trial> {
        loop {
            match self.phase {
                Phase::Done => return vec![],
                Phase::Start => {
                    let p = self.trial("start", self.out.list.clone(), None);
                    let t = p.trial.clone();
                    self.pending = vec![p];
                    return vec![t];
                }
                Phase::Support => {
                    if self.policy.mode == "off" {
                        self.out.stopped = "done".into();
                        self.phase = Phase::Done;
                        continue;
                    }
                    let has_used = self
                        .out
                        .relation
                        .as_ref()
                        .is_some_and(|r| r.seeds_used.is_some());
                    match self.support_list() {
                        Some(l) if self.budget_left() > 0 => {
                            let p = self.trial("support", l, None);
                            let t = p.trial.clone();
                            self.pending = vec![p];
                            return vec![t];
                        }
                        Some(_) => {
                            self.out.support = "budget".into();
                            self.out.stopped = "max_solves".into();
                            self.phase = Phase::Done;
                        }
                        None => {
                            self.out.support = if has_used {
                                "not_smaller".into()
                            } else {
                                "unavailable".into()
                            };
                            self.phase = Phase::Greedy;
                        }
                    }
                }
                Phase::Resupport => match self.support_list() {
                    Some(l) if self.budget_left() > 0 => {
                        let p = self.trial("resupport", l, None);
                        let t = p.trial.clone();
                        self.pending = vec![p];
                        return vec![t];
                    }
                    Some(_) => {
                        self.out.stopped = "max_solves".into();
                        self.phase = Phase::Done;
                    }
                    None => self.phase = Phase::Greedy,
                },
                Phase::Greedy => {
                    if self.policy.mode != "greedy" {
                        self.out.stopped = "done".into();
                        self.phase = Phase::Done;
                        continue;
                    }
                    if self.order.is_none() {
                        let idx = drop_order(&self.out.list, &self.out.centre, self.policy);
                        self.order =
                            Some(idx.into_iter().map(|i| self.out.list[i].clone()).collect());
                    }
                    let cur: BTreeSet<&Offset> = self.out.list.iter().collect();
                    let remaining: Vec<Offset> = self
                        .order
                        .as_ref()
                        .map(|o| {
                            o.iter()
                                .filter(|s| cur.contains(s) && !self.tried.contains(*s))
                                .cloned()
                                .collect()
                        })
                        .unwrap_or_default();
                    // a list of one seed is not reduced further (an empty list has no rows)
                    if remaining.is_empty() || self.out.list.len() <= 1 {
                        self.out.stopped = "done".into();
                        self.phase = Phase::Done;
                        continue;
                    }
                    let k = (self.policy.parallel_trials as u64).min(self.budget_left()) as usize;
                    if k == 0 {
                        self.out.stopped = "max_solves".into();
                        self.phase = Phase::Done;
                        continue;
                    }
                    let mut ps = vec![];
                    for s in remaining.into_iter().take(k) {
                        let l = without(&self.out.list, &s);
                        ps.push(self.trial("drop", l, Some(s)));
                    }
                    let ts = ps.iter().map(|p| p.trial.clone()).collect();
                    self.pending = ps;
                    return ts;
                }
            }
        }
    }

    /// Feed the answers of the last batch (one answer per trial: one key each).
    pub(crate) fn feed(&mut self, answers: Vec<Vec<TrialAnswer>>, system: &mut dyn SystemPoles) {
        let pending = std::mem::take(&mut self.pending);
        self.out.trials_run += pending.len() as u64;
        let mut ans: Vec<TrialAnswer> = answers
            .into_iter()
            .map(|mut v| {
                v.pop().unwrap_or(TrialAnswer::Undecided {
                    class: "missing".into(),
                    message: "no answer".into(),
                })
            })
            .collect();
        match self.phase {
            Phase::Start => {
                let a = ans.pop().expect("one start answer");
                let p = pending.into_iter().next().expect("one start trial");
                self.take_start(a, Some(&p));
            }
            Phase::Support | Phase::Resupport => {
                let a = ans.pop().expect("one answer");
                let p = pending.into_iter().next().expect("one trial");
                self.out.solves += 1;
                let (kept, reason) = self.judge(&p, &a, system);
                let what = if self.phase == Phase::Support {
                    "support"
                } else {
                    "resupport"
                };
                if kept {
                    self.out.list = p.list.clone();
                    self.out.relation = a.relation().cloned();
                }
                if self.phase == Phase::Support {
                    self.out.support = if kept {
                        "applied".into()
                    } else {
                        format!("refused: {reason}")
                    };
                }
                self.log(&p, &a, Some(kept), format!("{what}: {reason}"), true);
                self.phase = Phase::Greedy;
            }
            Phase::Greedy => {
                let mut first: Option<usize> = None;
                let mut verdicts: Vec<(bool, String)> = vec![];
                for (p, a) in pending.iter().zip(&ans) {
                    let v = self.judge(p, a, system);
                    let ok = v.0;
                    verdicts.push(v);
                    if ok {
                        first = Some(verdicts.len() - 1);
                        break;
                    }
                }
                let counted = first.map(|j| j + 1).unwrap_or(pending.len());
                self.out.solves += counted as u64;
                for (i, (p, a)) in pending.iter().zip(&ans).enumerate() {
                    if i < counted {
                        let (k, r) = verdicts[i].clone();
                        if let Some(s) = &p.removed {
                            self.tried.insert(s.clone());
                        }
                        self.log(p, a, Some(k), r, true);
                    } else {
                        self.log(
                            p,
                            a,
                            None,
                            "speculative: after the first kept drop".into(),
                            false,
                        );
                    }
                }
                if let Some(j) = first {
                    self.out.list = pending[j].list.clone();
                    self.out.relation = ans[j].relation().cloned();
                    if self.policy.resupport_after_drop {
                        self.phase = Phase::Resupport;
                    }
                }
            }
            Phase::Done => {}
        }
    }

    fn judge(&self, p: &Pending, a: &TrialAnswer, system: &mut dyn SystemPoles) -> (bool, String) {
        let Some(new) = a.relation() else {
            return (false, format!("answer {}", a.word()));
        };
        let Some(old) = self.out.relation.as_ref() else {
            return (false, "no current relation".into());
        };
        let k = &self.out.key;
        let v = judge_change(
            &[State { key: k, facts: old }],
            &[State { key: k, facts: new }],
            &self.out.list,
            &p.list,
            &self.out.centre,
            self.policy,
            system,
        );
        (v.keep, v.reason)
    }

    /// The result (with the contract's `minimise` member).
    pub(crate) fn finish(mut self) -> KeyMin {
        if let (Some(start), Some(rel)) = (&self.out.start_relation, &self.out.relation)
            && self.policy.mode != "off"
            && (self.out.stopped == "done" || self.out.stopped == "max_solves")
        {
            self.out.minimise = Some(json!({
                "mode": self.policy.mode, "measure": self.policy.measure, "order": self.policy.order,
                "solves": self.out.solves, "stopped": self.out.stopped,
                "seeds_before": self.out.seeds_before, "seeds_after": self.out.list.len(),
                "rhs_before": start.rhs_count, "rhs_after": rel.rhs_count,
                "original_relation_digest": start.relation_digest}));
        }
        self.out
    }
}

/// Drive several machines in lockstep: each round collects the next batch of every active machine and solves them in
/// ONE solver call (independent trials: the supervisor may run them in parallel).
pub(crate) fn run_lockstep(
    machines: &mut [KeyMachine<'_>],
    solver: &mut dyn TrialSolver,
    system: &mut dyn SystemPoles,
) -> Result<(), SeedMinError> {
    loop {
        let mut all: Vec<Trial> = vec![];
        let mut spans: Vec<(usize, usize)> = vec![];
        for (i, m) in machines.iter_mut().enumerate() {
            let b = m.next_batch();
            if !b.is_empty() {
                spans.push((i, b.len()));
                all.extend(b);
            }
        }
        if all.is_empty() {
            return Ok(());
        }
        let mut answers = run_batch(solver, &all)?.into_iter();
        for (i, n) in spans {
            let part: Vec<Vec<TrialAnswer>> = answers.by_ref().take(n).collect();
            machines[i].feed(part, system);
        }
    }
}

/// Minimise the seed list `list` (centre-relative) at one key (interfaces/seedmin.md 4).
pub fn minimise_key(
    key: &[i32],
    centre: &[i32],
    list: &[Offset],
    policy: &SeedMinPolicy,
    solver: &mut dyn TrialSolver,
    system: &mut dyn SystemPoles,
) -> Result<KeyMin, SeedMinError> {
    check_input(std::slice::from_ref(&key.to_vec()), centre, list)?;
    preflight(policy, solver)?;
    let ordered = row_ordered(list, centre, policy);
    let list = ordered.as_slice();
    let label = format!("seedmin/key{}", key_label(key));
    let mut km = if policy.backend == "engine" {
        engine_key(key, centre, list, policy, solver, &label)?
    } else {
        let mut ms = vec![KeyMachine::new(
            key,
            centre,
            list,
            policy,
            label.clone(),
            None,
        )];
        run_lockstep(&mut ms, solver, system)?;
        ms.pop().expect("one machine").finish()
    };
    if policy.rows_at_d0 {
        let start = km.start_relation.is_some();
        let d0 = super::joint::rows_at_d0_pass(
            std::slice::from_ref(&km.key),
            centre,
            list,
            &km.list,
            km.relation.as_ref().map(|r| vec![r.clone()]),
            policy,
            solver,
            system,
            &label,
        )?;
        if let Some((l, facts)) = d0.adopted.clone()
            && start
        {
            km.list = l;
            km.relation = facts.into_iter().next();
        }
        km.rows_at_d0 = Some(d0.record);
    }
    Ok(km)
}

/// Shared input checks: non-empty list, one arity everywhere, no duplicate offsets or keys.
pub(crate) fn check_input(
    keys: &[Key],
    centre: &[i32],
    list: &[Offset],
) -> Result<(), SeedMinError> {
    let bad = |m: String| Err(SeedMinError::Input { message: m });
    let n = centre.len();
    if list.is_empty() {
        return bad("empty seed list".into());
    }
    if keys.is_empty() {
        return bad("no keys".into());
    }
    if keys.iter().any(|k| k.len() != n) || list.iter().any(|o| o.len() != n) {
        return bad(format!("arity mismatch (centre has {n} slots)"));
    }
    let s: BTreeSet<&Offset> = list.iter().collect();
    if s.len() != list.len() {
        return bad("duplicate offsets in the seed list".into());
    }
    let ks: BTreeSet<&Key> = keys.iter().collect();
    if ks.len() != keys.len() {
        return bad("duplicate keys".into());
    }
    Ok(())
}

/// Capability checks before any solve (typed refusals).
pub(crate) fn preflight(
    policy: &SeedMinPolicy,
    solver: &dyn TrialSolver,
) -> Result<(), SeedMinError> {
    policy
        .validate()
        .map_err(|message| SeedMinError::Input { message })?;
    let caps = solver.capabilities();
    if policy.rows_at_d0 && !(caps.probe_d || caps.discovery_d) {
        return Err(SeedMinError::EngineLacksProbeMode {
            features: caps.features,
        });
    }
    if policy.backend == "engine" && !caps.minimise {
        return Err(SeedMinError::EngineLacksMinimiser {
            features: caps.features,
        });
    }
    Ok(())
}

/// Backend `engine`: ONE trial whose recipe carries `elimination.minimise`; the record's `minimise` member is taken
/// verbatim; the minimised list is `minimise.seeds` (offsets from the key; PROPOSED contract member) or else the
/// relation's support.
fn engine_key(
    key: &[i32],
    centre: &[i32],
    list: &[Offset],
    policy: &SeedMinPolicy,
    solver: &mut dyn TrialSolver,
    label: &str,
) -> Result<KeyMin, SeedMinError> {
    let t = Trial {
        label: format!("{label}/engine"),
        centre: centre.to_vec(),
        list: list.to_vec(),
        keys: vec![key.to_vec()],
        mode: TrialMode::Solve,
        engine_minimise: Some(policy.engine_member()),
    };
    let a = run_batch(solver, std::slice::from_ref(&t))?
        .pop()
        .and_then(|mut v| v.pop())
        .expect("one answer");
    let mut km = KeyMin {
        key: key.to_vec(),
        centre: centre.to_vec(),
        backend: "engine".into(),
        row_order: policy.row_order.clone(),
        list: list.to_vec(),
        relation: None,
        start_relation: None,
        seeds_before: list.len(),
        stopped: String::new(),
        support: "engine".into(),
        minimise: None,
        solves: 0,
        trials_run: 1,
        trials: vec![TrialLog {
            label: t.label.clone(),
            digest: t.digest(),
            kind: "engine".into(),
            removed: None,
            keys: 1,
            seeds: list.len(),
            answers: vec![a.word()],
            kept: None,
            reason: "engine minimiser".into(),
            counted: false,
        }],
        rows_at_d0: None,
    };
    match a {
        TrialAnswer::Relation(r) => {
            let m = r.minimise.clone().unwrap_or(Value::Null);
            let seeds: Option<Vec<Offset>> = m
                .get("seeds")
                .and_then(|s| serde_json::from_value::<Vec<Offset>>(s.clone()).ok())
                .map(|v| v.iter().map(|o| sub(o, centre)).collect());
            km.list = match seeds.or_else(|| r.seeds_used.clone()) {
                Some(l) => restrict(list, &l),
                None => list.to_vec(),
            };
            km.stopped = m["stopped"].as_str().unwrap_or("done").to_string();
            km.solves = m["solves"].as_u64().unwrap_or(0);
            km.minimise = (!m.is_null()).then_some(m);
            km.relation = Some(*r);
        }
        TrialAnswer::NoRelation | TrialAnswer::NotInDomain => km.stopped = "no_start".into(),
        _ => km.stopped = "undecided_start".into(),
    }
    Ok(km)
}
