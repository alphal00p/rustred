//! SEED MINIMISATION behind the contract's line-7 names (`elimination.minimise {mode, order, max_solves, measure}`;
//! coord-seed-minimise.md 5-8). Backend `engine`: the recipe carries the member and ONE solve returns the minimised
//! relation (the engine minimiser: not coded yet). Backend `emulated` (DEVIATION, the default until it exists): repeated
//! solves through the [`CoreSolver`]:
//! 1. the list in the policy's order (`distance`: L1 distance of the seed from the key, ties generation order) is
//!    solved at the domain keys (or its answers are given);
//! 2. SUPPORT: restricted to the seeds with a nonzero proof weight (`seeds_used`; union over the domain keys) and
//!    re-solved;
//! 3. GREEDY (delta debugging, the helper's procedure): drop a chunk of the current list; keep the drop iff the trial is
//!    certified at EVERY domain key and passes the policy checks (measure, `monotone_rhs`, `keep_regular_at_d0`), then
//!    restrict to the trial's support; chunks halve down to single seeds; `max_solves` bounds the trials (a count).
//!    `parallel_trials` speculative drops are solved at once and the FIRST success in order wins (the later ones are
//!    discarded): the result is that of the sequential procedure, whatever the number of CPUs. A budget answer counts as
//!    a failed drop (the seeds are kept; the list stays valid) and is counted.
//!
//! Joint domains (many keys): the keys that failed the latest trials are solved first (`witness_keys`), the rest only
//! if those pass: cost only, the decision is "certified at every key".

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::seeds::{Offset, restrict};
use super::spec::CoreSeedPolicy;
use super::{CoreSolver, Key, KeyAnswer, RelationInfo, SolveReq};

/// Builds the request of a list at keys (the rule's recipe with `list` as its explicit centres).
pub type MakeReq<'a> =
    dyn Fn(&[Offset], &[Key], &str, Option<Value>) -> Result<SolveReq, String> + 'a;

/// What a minimisation produced.
#[derive(Clone, Debug)]
pub struct MinResult {
    /// The final list (in the policy's order).
    pub list: Vec<Offset>,
    /// The certified relations of the final list at the domain keys.
    pub per_key: BTreeMap<Key, RelationInfo>,
    /// The line-7 `minimise` member (+ emulation fields); deterministic (the sequential procedure's numbers).
    pub member: Value,
    /// Cost only (depends on `parallel_trials` and staging): trials solved in total, speculative trials discarded,
    /// failed drops whose failing key was a budget answer.
    pub cost: Value,
}

/// Outcome of one trial list at the domain.
struct Trial {
    feasible: bool,
    per_key: BTreeMap<Key, RelationInfo>,
    failed_key: Option<Key>,
    budget: bool,
}

fn digest_lines(lines: &[String]) -> String {
    let mut h = crate::closure::sha256::Sha256::new();
    for l in lines {
        h.update(l.as_bytes());
        h.update(b"\n");
    }
    h.finish_hex()
}

/// Policy checks of a candidate against the current relations: certified at every key, the measure, monotone size,
/// regularity kept.
fn judge(
    policy: &CoreSeedPolicy,
    keys: &[Key],
    prev: &BTreeMap<Key, RelationInfo>,
    answers: &BTreeMap<Key, KeyAnswer>,
) -> Trial {
    let fail = |k: &Key, budget: bool| Trial {
        feasible: false,
        per_key: BTreeMap::new(),
        failed_key: Some(k.clone()),
        budget,
    };
    // the first failing SOLVED key in key order (a staged trial solved only its witness keys)
    for k in keys {
        match answers.get(k) {
            None | Some(KeyAnswer::Relation(_)) => {}
            Some(a) => return fail(k, matches!(a, KeyAnswer::Budget { .. })),
        }
    }
    let mut per_key = BTreeMap::new();
    for k in keys {
        let Some(KeyAnswer::Relation(r)) = answers.get(k) else {
            return fail(k, false);
        };
        if policy.minimise.measure == "m1"
            && r.relation_rule_taken.as_deref().is_some_and(|t| t != "m1")
        {
            return fail(k, false);
        }
        if policy.minimise.keep_regular_at_d0
            && prev.get(k).is_some_and(|p| p.d0.is_regular())
            && !r.d0.is_regular()
        {
            return fail(k, false);
        }
        per_key.insert(k.clone(), (**r).clone());
    }
    if policy.minimise.monotone_rhs && !prev.is_empty() {
        let s_new: u64 = per_key.values().map(|r| r.rhs_count).sum();
        let s_old: u64 = prev.values().map(|r| r.rhs_count).sum();
        if s_new > s_old {
            return Trial {
                feasible: false,
                per_key: BTreeMap::new(),
                failed_key: None,
                budget: false,
            };
        }
    }
    Trial {
        feasible: true,
        per_key,
        failed_key: None,
        budget: false,
    }
}

/// The support of a certified trial: union of `seeds_used` over the keys (None if some key does not report it).
fn support(per_key: &BTreeMap<Key, RelationInfo>) -> Option<BTreeSet<Offset>> {
    let mut s = BTreeSet::new();
    for r in per_key.values() {
        s.extend(r.seeds_used.as_ref()?.iter().cloned());
    }
    Some(s)
}

/// Solve a batch of trial lists at `keys`, staged by the witness keys; returns the answers per trial.
fn solve_trials(
    solver: &mut dyn CoreSolver,
    make: &MakeReq<'_>,
    lists: &[Vec<Offset>],
    keys: &[Key],
    witness: &[Key],
    label: &str,
) -> Result<Vec<BTreeMap<Key, KeyAnswer>>, String> {
    let w: Vec<Key> = witness
        .iter()
        .filter(|k| keys.contains(k))
        .cloned()
        .collect();
    let rest: Vec<Key> = keys.iter().filter(|k| !w.contains(k)).cloned().collect();
    let staged = !w.is_empty() && !rest.is_empty();
    let first_keys = if staged { w.clone() } else { keys.to_vec() };
    let reqs: Vec<SolveReq> = lists
        .iter()
        .enumerate()
        .map(|(i, l)| make(l, &first_keys, &format!("{label}/t{i}"), None))
        .collect::<Result<_, _>>()?;
    let ans = solver.solve(&reqs)?;
    let mut out: Vec<BTreeMap<Key, KeyAnswer>> = ans
        .into_iter()
        .map(|a| first_keys.iter().cloned().zip(a).collect())
        .collect();
    if staged {
        let pass: Vec<usize> = (0..lists.len())
            .filter(|&i| out[i].values().all(|a| matches!(a, KeyAnswer::Relation(_))))
            .collect();
        if !pass.is_empty() {
            let reqs2: Vec<SolveReq> = pass
                .iter()
                .map(|&i| make(&lists[i], &rest, &format!("{label}/t{i}/rest"), None))
                .collect::<Result<_, _>>()?;
            let ans2 = solver.solve(&reqs2)?;
            for (&i, a) in pass.iter().zip(ans2) {
                out[i].extend(rest.iter().cloned().zip(a));
            }
        }
    }
    Ok(out)
}

/// Minimise `list` (already in the policy's order) on the domain `keys` (one key: a per-key list; several: JOINT).
/// `known` = the answers of `list` itself at `keys` when already solved. Returns None when `list` is not certified at
/// every domain key.
pub fn minimise(
    solver: &mut dyn CoreSolver,
    policy: &CoreSeedPolicy,
    make: &MakeReq<'_>,
    list: &[Offset],
    keys: &[Key],
    known: Option<&BTreeMap<Key, RelationInfo>>,
    label: &str,
) -> Result<Option<MinResult>, String> {
    let m = &policy.minimise;
    let joint = keys.len() > 1;
    // backend engine: one solve with the member in the recipe (per-key minimisation of the engine; a joint domain is
    // emulated, the engine has no joint mode)
    if m.backend == "engine" && !joint && m.mode != "off" {
        let member = json!({"mode": m.mode, "order": m.order, "max_solves": m.max_solves, "measure": m.measure});
        let req = make(list, keys, &format!("{label}/engine"), Some(member))?;
        let ans = solver.solve(std::slice::from_ref(&req))?;
        let Some(KeyAnswer::Relation(r)) = ans.first().and_then(|a| a.first()) else {
            return Ok(None);
        };
        let used: BTreeSet<Offset> = r
            .seeds_used
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let mut mem = r.minimise.clone().unwrap_or(Value::Null);
        mem["backend"] = json!("engine");
        return Ok(Some(MinResult {
            list: restrict(list, &used),
            per_key: [(keys[0].clone(), (**r).clone())].into_iter().collect(),
            member: mem,
            cost: json!({"trials_solved": 1}),
        }));
    }
    let mut solves = 0u64;
    let mut budget_trials = 0u64;
    let mut discarded = 0u64;
    let mut solved_total = 0u64;
    let base: BTreeMap<Key, RelationInfo> = match known {
        Some(k) if keys.iter().all(|x| k.contains_key(x)) => {
            keys.iter().map(|x| (x.clone(), k[x].clone())).collect()
        }
        _ => {
            let a = solve_trials(
                solver,
                make,
                &[list.to_vec()],
                keys,
                &[],
                &format!("{label}/full"),
            )?;
            solves += 1;
            solved_total += 1;
            let t = judge(policy, keys, &BTreeMap::new(), &a[0]);
            if !t.feasible {
                return Ok(None);
            }
            t.per_key
        }
    };
    let rhs_of = |p: &BTreeMap<Key, RelationInfo>| -> (u64, u64) {
        (
            p.values().map(|r| r.rhs_count).max().unwrap_or(0),
            p.values().map(|r| r.rhs_count).sum(),
        )
    };
    let original = digest_lines(
        &base
            .iter()
            .map(|(k, r)| format!("{k:?} {}", r.relation_digest))
            .collect::<Vec<_>>(),
    );
    let original = if joint {
        original
    } else {
        base.values()
            .next()
            .map(|r| r.relation_digest.clone())
            .unwrap_or(original)
    };
    let (rhs_before, rhs_sum_before) = rhs_of(&base);
    let mut keep: Vec<Offset> = list.to_vec();
    let mut cur = base.clone();
    // the last list whose relations `cur` are (the final list may be a support restriction of it)
    let mut solved_list: Vec<Offset> = keep.clone();
    let mut stopped = "done";
    if m.mode != "off"
        && let Some(s) = support(&cur)
    {
        let r = restrict(&keep, &s);
        if r.len() < keep.len() && !r.is_empty() && solves < m.max_solves {
            let a = solve_trials(
                solver,
                make,
                std::slice::from_ref(&r),
                keys,
                &[],
                &format!("{label}/support"),
            )?;
            solves += 1;
            solved_total += 1;
            let t = judge(policy, keys, &cur, &a[0]);
            budget_trials += t.budget as u64;
            if t.feasible {
                keep = r;
                cur = t.per_key;
                solved_list = keep.clone();
            }
        }
    }
    let mut witness: Vec<Key> = vec![];
    if m.mode == "greedy" {
        let p = m.parallel_trials.max(1);
        let mut chunk = (keep.len() / 2).max(1);
        let mut pos = 0usize;
        loop {
            if pos >= keep.len() {
                if chunk == 1 {
                    break;
                }
                chunk = (chunk / 2).max(1);
                pos = 0;
                continue;
            }
            if solves >= m.max_solves {
                stopped = "max_solves";
                break;
            }
            // up to p trials at positions pos, pos + chunk, ... against the current list
            let mut slots: Vec<(usize, Option<Vec<Offset>>)> = vec![];
            let mut q = pos;
            let mut n_solve = 0u64;
            while slots.len() < p && q < keep.len() && solves + n_solve < m.max_solves {
                let t: Vec<Offset> = keep[..q]
                    .iter()
                    .chain(keep[(q + chunk).min(keep.len())..].iter())
                    .cloned()
                    .collect();
                if t.is_empty() {
                    slots.push((q, None));
                } else {
                    slots.push((q, Some(t)));
                    n_solve += 1;
                }
                q += chunk;
            }
            let lists: Vec<Vec<Offset>> = slots.iter().filter_map(|s| s.1.clone()).collect();
            let answers = if lists.is_empty() {
                vec![]
            } else {
                solve_trials(
                    solver,
                    make,
                    &lists,
                    keys,
                    &witness,
                    &format!("{label}/c{chunk}p{pos}"),
                )?
            };
            solved_total += lists.len() as u64;
            let mut it = answers.into_iter();
            let mut applied = false;
            let mut used = 0u64;
            for (qj, t) in &slots {
                let Some(t) = t else {
                    pos = qj + chunk;
                    continue;
                };
                let a = it.next().unwrap_or_default();
                used += 1;
                let tr = judge(policy, keys, &cur, &a);
                budget_trials += tr.budget as u64;
                if tr.feasible {
                    let s = support(&tr.per_key);
                    keep = match s {
                        Some(s) => restrict(t, &s),
                        None => t.clone(),
                    };
                    solved_list = t.clone();
                    cur = tr.per_key;
                    pos = *qj;
                    applied = true;
                    break;
                }
                if let Some(k) = tr.failed_key {
                    witness.retain(|x| *x != k);
                    witness.insert(0, k);
                    witness.truncate(policy.joint.witness_keys.max(1));
                }
                pos = qj + chunk;
            }
            // the sequential procedure would have run exactly the trials up to the first success
            solves += used;
            discarded += lists.len() as u64 - used;
            let _ = applied;
        }
    }
    // the relations of the FINAL list (a support restriction of the last solved list): solve it once more
    if keep != solved_list {
        let a = solve_trials(
            solver,
            make,
            std::slice::from_ref(&keep),
            keys,
            &witness,
            &format!("{label}/final"),
        )?;
        solves += 1;
        solved_total += 1;
        let t = judge(policy, keys, &cur, &a[0]);
        budget_trials += t.budget as u64;
        if t.feasible {
            cur = t.per_key;
        } else {
            keep = solved_list.clone();
        }
    }
    let (rhs_after, rhs_sum_after) = rhs_of(&cur);
    let member = json!({"mode": m.mode, "measure": m.measure, "order": m.order, "backend": "emulated",
        "solves": solves, "stopped": stopped, "seeds_before": list.len(), "seeds_after": keep.len(),
        "rhs_before": rhs_before, "rhs_after": rhs_after, "rhs_sum_before": rhs_sum_before,
        "rhs_sum_after": rhs_sum_after, "original_relation_digest": original, "domain_keys": keys.len()});
    let cost = json!({"trials_solved": solved_total, "speculative_discarded": discarded,
        "budget_trials": budget_trials, "parallel_trials": m.parallel_trials});
    Ok(Some(MinResult {
        list: keep,
        per_key: cur,
        member,
        cost,
    }))
}
