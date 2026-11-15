//! BRIDGE for lane coreseed: its former `coreseed::minimise::minimise` (same signature, same `MinResult`, same record
//! member names) implemented on seedmin, so that ONE minimiser is in the tree: coreseed switches its call site by an
//! import (`use crate::seedmin::bridge::{MakeReq, minimise};`) and deletes its own file. Its `CoreSolver` is wrapped as a
//! [`TrialSolver`]; its policy's `minimise` block maps onto [`SeedMinPolicy`] (same names: backend, mode, order, measure,
//! max_solves, monotone_rhs, parallel_trials, rows_at_d0; `keep_regular_at_d0: false` = `pole_objective: off`).
//! Lists are offsets FROM THE KEY (centre = the zero offset). One key: the per-key machine; several keys: the joint
//! minimisation, returning the piece that covers every key (else the start list: the result stays valid at every key).

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::joint::minimise_joint;
use super::key::{KeyMachine, run_lockstep};
use super::policy::{JointPolicy, SeedMinPolicy};
use super::{
    EngineCaps, Key, Offset, PoleOrder, RelationFacts, StubSystemPoles, Trial, TrialAnswer,
    TrialSolver, key_label,
};
use crate::coreseed::spec::CoreSeedPolicy;
use crate::coreseed::{CoreSolver, D0Verdict, KeyAnswer, RelationInfo, SolveReq};

/// Builds the request of a list at keys (the rule's recipe with `list` as its explicit centres; the last argument is
/// the engine backend's `elimination.minimise` member).
pub type MakeReq<'a> =
    dyn Fn(&[Offset], &[Key], &str, Option<Value>) -> Result<SolveReq, String> + 'a;

/// What a minimisation produced (coreseed's former type, unchanged).
#[derive(Clone, Debug)]
pub struct MinResult {
    /// The final list (row order of the input list).
    pub list: Vec<Offset>,
    /// The certified relations of the final list at the domain keys.
    pub per_key: BTreeMap<Key, RelationInfo>,
    /// The line-7 `minimise` member + emulation fields (backend, rhs_sum_before / after, domain_keys, row_order).
    pub member: Value,
    /// Cost only: trials solved, speculative trials discarded.
    pub cost: Value,
}

/// coreseed's policy block as a seedmin policy.
pub fn policy_from_core(p: &CoreSeedPolicy) -> SeedMinPolicy {
    let m = &p.minimise;
    SeedMinPolicy {
        backend: m.backend.clone(),
        mode: m.mode.clone(),
        order: m.order.clone(),
        measure: m.measure.clone(),
        max_solves: m.max_solves,
        monotone_rhs: m.monotone_rhs,
        parallel_trials: m.parallel_trials.max(1),
        rows_at_d0: m.rows_at_d0,
        pole_objective: if m.keep_regular_at_d0 { "on" } else { "off" }.into(),
        d0: p.d0.clone(),
        joint: JointPolicy {
            screen_keys: p.joint.witness_keys.max(1),
            ..JointPolicy::default()
        },
        ..SeedMinPolicy::default()
    }
}

/// coreseed's d0 verdict as a pole order.
pub fn pole_of(d: &D0Verdict) -> PoleOrder {
    match d {
        D0Verdict::Regular => PoleOrder::regular(),
        D0Verdict::Singular { order: Some(o) } => PoleOrder::Order { order: *o },
        D0Verdict::Singular { order: None } => PoleOrder::Singular,
        D0Verdict::Undecided => PoleOrder::Undecided,
    }
}

fn facts_of(r: &RelationInfo) -> RelationFacts {
    RelationFacts {
        relation_digest: r.relation_digest.clone(),
        rhs_count: r.rhs_count,
        rhs_sub_count: None,
        bytes: None,
        trace_rows: None,
        seeds_used: r.seeds_used.clone(),
        pole: pole_of(&r.d0),
        minimise: r.minimise.clone(),
    }
}

/// A coreseed solver as a seedmin solver; remembers the full `RelationInfo` of every relation it saw.
struct Bridge<'s, 'm, 'a> {
    solver: &'s mut dyn CoreSolver,
    make: &'m MakeReq<'a>,
    infos: BTreeMap<(Key, String), RelationInfo>,
    engine: bool,
    trials: u64,
}

impl TrialSolver for Bridge<'_, '_, '_> {
    fn solve(&mut self, trials: &[Trial]) -> Result<Vec<Vec<TrialAnswer>>, String> {
        let reqs: Vec<SolveReq> = trials
            .iter()
            .map(|t| {
                (self.make)(
                    &t.key_offsets(),
                    &t.keys,
                    &t.label,
                    t.engine_minimise.clone(),
                )
            })
            .collect::<Result<_, _>>()?;
        self.trials += reqs.len() as u64;
        let ans = self.solver.solve(&reqs)?;
        let mut out = vec![];
        for (t, a) in trials.iter().zip(ans) {
            let mut v = vec![];
            for (k, x) in t.keys.iter().zip(a) {
                v.push(match x {
                    KeyAnswer::Relation(r) => {
                        let mut f = facts_of(&r);
                        // coreseed reports the support as offsets FROM THE KEY: centre-relative for the minimiser
                        f.seeds_used = f
                            .seeds_used
                            .map(|v| v.iter().map(|o| super::sub(o, &t.centre)).collect());
                        self.infos
                            .insert((k.clone(), r.relation_digest.clone()), *r);
                        TrialAnswer::Relation(Box::new(f))
                    }
                    KeyAnswer::NoRelation => TrialAnswer::NoRelation,
                    KeyAnswer::NotInDomain => TrialAnswer::NotInDomain,
                    KeyAnswer::Budget { what } => TrialAnswer::Undecided {
                        class: "budget".into(),
                        message: what,
                    },
                    KeyAnswer::Failed { class, message } => {
                        TrialAnswer::Undecided { class, message }
                    }
                });
            }
            out.push(v);
        }
        Ok(out)
    }

    fn capabilities(&self) -> EngineCaps {
        EngineCaps {
            probe_d: false,
            discovery_d: false,
            minimise: self.engine,
            features: vec![],
        }
    }
}

/// coreseed's former `minimise` (same signature and contract): minimise `list` (offsets from the key, in its row
/// order) on the domain `keys`; `known` = the answers of `list` itself at `keys` when already solved. `None` when
/// `list` is not certified at every domain key.
pub fn minimise(
    solver: &mut dyn CoreSolver,
    policy: &CoreSeedPolicy,
    make: &MakeReq<'_>,
    list: &[Offset],
    keys: &[Key],
    known: Option<&BTreeMap<Key, RelationInfo>>,
    label: &str,
) -> Result<Option<MinResult>, String> {
    let sp = policy_from_core(policy);
    let n = list.first().map(|o| o.len()).unwrap_or(0);
    let centre = vec![0; n];
    let mut br = Bridge {
        solver,
        make,
        infos: BTreeMap::new(),
        engine: sp.backend == "engine",
        trials: 0,
    };
    let mut sys = StubSystemPoles;
    let info_of = |br: &Bridge<'_, '_, '_>, k: &Key, f: &RelationFacts| -> Option<RelationInfo> {
        br.infos
            .get(&(k.clone(), f.relation_digest.clone()))
            .cloned()
    };
    if keys.len() == 1 {
        let k = &keys[0];
        let start = known.and_then(|m| m.get(k)).map(|r| {
            br.infos
                .insert((k.clone(), r.relation_digest.clone()), r.clone());
            TrialAnswer::Relation(Box::new(facts_of(r)))
        });
        let km = if sp.backend == "engine" || sp.rows_at_d0 {
            super::key::minimise_key(k, &centre, list, &sp, &mut br, &mut sys)
                .map_err(|e| e.to_string())?
        } else {
            let mut ms = vec![KeyMachine::new(
                k,
                &centre,
                list,
                &sp,
                format!("{label}/seedmin/key{}", key_label(k)),
                start.as_ref(),
            )];
            run_lockstep(&mut ms, &mut br, &mut sys).map_err(|e| e.to_string())?;
            ms.pop().expect("one machine").finish()
        };
        // the engine backend's record carries no start relation, only its `minimise` member (rhs_before)
        let Some(rel) = &km.relation else {
            return Ok(None);
        };
        let Some(info) = info_of(&br, k, rel) else {
            return Ok(None);
        };
        let mut member = km.minimise.clone().unwrap_or_else(|| json!({}));
        let rhs_before = km
            .start_relation
            .as_ref()
            .map(|s| s.rhs_count)
            .or_else(|| member["rhs_before"].as_u64());
        member["backend"] = json!(km.backend);
        member["rhs_sum_before"] = json!(rhs_before);
        member["rhs_sum_after"] = json!(rel.rhs_count);
        member["domain_keys"] = json!(1);
        member["row_order"] = json!(sp.row_order);
        return Ok(Some(MinResult {
            list: km.list.clone(),
            per_key: [(k.clone(), info)].into_iter().collect(),
            member,
            cost: json!({"trials_solved": br.trials, "trials_run": km.trials_run}),
        }));
    }
    let mut sp1 = sp.clone();
    sp1.joint.max_pieces = 1;
    sp1.joint.choice_sets_complete = false;
    let jm =
        minimise_joint(keys, &centre, list, &sp1, &mut br, &mut sys).map_err(|e| e.to_string())?;
    if !jm.unserved.is_empty() || !jm.undecided.is_empty() {
        return Ok(None);
    }
    let p = match jm.pieces.first() {
        Some(p) if p.keys.len() == keys.len() => p.clone(),
        _ => {
            // no single list covers every key: the start list (certified at every key by the start trial)
            jm.pieces
                .iter()
                .find(|p| p.how == "start_list" && p.keys.len() == keys.len())
                .cloned()
                .unwrap_or_else(|| jm.pieces[0].clone())
        }
    };
    let mut per_key = BTreeMap::new();
    for (k, f) in p.keys.iter().zip(&p.per_key) {
        if let Some(i) = info_of(&br, k, f) {
            per_key.insert(k.clone(), i);
        }
    }
    let mut member = p.minimise.clone().unwrap_or_else(|| json!({}));
    member["backend"] = json!(sp.backend);
    member["rhs_sum_after"] = json!(p.per_key.iter().map(|f| f.rhs_count).sum::<u64>());
    member["domain_keys"] = json!(keys.len());
    member["row_order"] = json!(sp.row_order);
    Ok(Some(MinResult {
        list: p.list,
        per_key,
        member,
        cost: json!({"trials_solved": br.trials, "trials_run": jm.trials_run}),
    }))
}

/// coreseed's CoreSolver as a seedmin solver for any of the minimiser's entries (the adapter of [`minimise`]).
fn with_bridge<T>(
    solver: &mut dyn CoreSolver,
    make: &MakeReq<'_>,
    engine: bool,
    f: impl FnOnce(&mut dyn TrialSolver) -> Result<T, super::SeedMinError>,
) -> Result<(T, BTreeMap<(Key, String), RelationInfo>), String> {
    let mut br = Bridge {
        solver,
        make,
        infos: BTreeMap::new(),
        engine,
        trials: 0,
    };
    let out = f(&mut br).map_err(|e| e.to_string())?;
    Ok((out, br.infos))
}

/// JOINT minimisation through coreseed's CoreSolver (public entry for its flow): `centre` = one offset from the key,
/// `list` centre-relative in its row order; returns the seedmin result and the full `RelationInfo` of every relation
/// met (keyed by (key, relation digest)) so the caller keeps its own record type.
pub fn minimise_joint_core(
    solver: &mut dyn CoreSolver,
    policy: &CoreSeedPolicy,
    make: &MakeReq<'_>,
    keys: &[Key],
    centre: &[i32],
    list: &[Offset],
) -> Result<(super::JointMin, BTreeMap<(Key, String), RelationInfo>), String> {
    let sp = policy_from_core(policy);
    let engine = sp.backend == "engine";
    with_bridge(solver, make, engine, |s| {
        minimise_joint(keys, centre, list, &sp, s, &mut StubSystemPoles)
    })
}

/// SHELL SUBSETS through coreseed's CoreSolver (public entry for its flow, implementer 2026-11-15): the start lists
/// base + each non-empty subset of `escalation` around `centre` (order = the seedmin policy's `shell_subsets`, default
/// smallest_first; `off` = the full set only), each minimised jointly; alternatives ranked best first, plus the
/// `RelationInfo` of every relation met.
#[allow(clippy::too_many_arguments)]
pub fn minimise_shell_subsets_core(
    solver: &mut dyn CoreSolver,
    policy: &CoreSeedPolicy,
    make: &MakeReq<'_>,
    keys: &[Key],
    centre: &[i32],
    lines: &[bool],
    base: &[String],
    escalation: &[String],
) -> Result<
    (
        Vec<super::SubsetAlternative>,
        BTreeMap<(Key, String), RelationInfo>,
    ),
    String,
> {
    let sp = policy_from_core(policy);
    let engine = sp.backend == "engine";
    let spec = super::ShellSubsetSpec::from_policy(base, escalation, &sp).unwrap_or_else(|| {
        // off: the full set only
        super::ShellSubsetSpec {
            base: base.iter().chain(escalation).cloned().collect(),
            escalation: vec![],
            order: "smallest_first".into(),
            rank: vec![],
        }
    });
    if spec.escalation.is_empty() {
        let list = super::subsets::subset_list(keys, centre, lines, &spec.base)?;
        let (j, infos) = with_bridge(solver, make, engine, |s| {
            minimise_joint(keys, centre, &list, &sp, s, &mut StubSystemPoles)
        })?;
        let alt = super::SubsetAlternative {
            shells: spec.base.clone(),
            start_seeds: list.len(),
            start_list_sha256: super::list_digest(&list),
            measures: serde_json::Value::Null,
            joint: j,
        };
        return Ok((vec![alt], infos));
    }
    with_bridge(solver, make, engine, |s| {
        super::minimise_shell_subsets(keys, centre, lines, &spec, &sp, s, &mut StubSystemPoles)
    })
}
