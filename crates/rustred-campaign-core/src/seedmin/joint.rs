//! JOINT minimisation over a domain (interfaces/seedmin.md 5; TRICKS 5 / 7): a centre-relative list is valid for a
//! domain only if the solve + exact proof succeeds at EVERY key of it. Spread keys minimised one by one (lockstep),
//! union, every-key confirmation, augmentation / repair / split, joint greedy screened on a rotating subset and
//! CONFIRMED ON EVERY KEY before a drop is kept, final verification, pieces, choice sets for lane regions, and the
//! diagnostic rows-at-d0 pass (TRICKS 14 c / 16).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::accept::{State, judge_change};
use super::key::{KeyMachine, KeyMin, TrialLog, check_input, preflight, run_lockstep};
use super::order::{drop_order, row_ordered, union_in_order, without, without_all};
use super::policy::SeedMinPolicy;
use super::{
    Key, Offset, PoleOrder, RelationFacts, SeedMinError, SystemPoles, Trial, TrialAnswer,
    TrialMode, TrialSolver, key_label, list_digest, run_batch, sha256_hex,
};

/// One piece of a domain: the keys a list is certified at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Piece {
    pub label: String,
    pub keys: Vec<Key>,
    /// Centre-relative, list order.
    pub list: Vec<Offset>,
    /// The final verification's relation per key (same order as `keys`).
    pub per_key: Vec<RelationFacts>,
    /// `joint` | `start_list` (keys left after `max_pieces`: the start list, certified at step 1).
    pub how: String,
    /// The contract's line-7 `minimise` member of the piece (joint: `original_relation_digest` = sha256 of the start
    /// relations' digests of the piece keys, in key order; rhs = maxima over the keys).
    pub minimise: Option<Value>,
    pub domain_tested: Value,
    #[serde(default)]
    pub rows_at_d0: Option<Value>,
}

/// The result of a joint minimisation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JointMin {
    pub pieces: Vec<Piece>,
    /// Keys where the START list has no relation (decided): in no piece.
    pub unserved: Vec<Key>,
    /// Keys where the start list's answer is undecided (budget / error): in no piece, never "no relation".
    pub undecided: Vec<(Key, String)>,
    /// lane regions' input form: {"signature", "keys": [{"key", "acceptable": [labels]}]}.
    pub choice_sets: Value,
    pub record: Value,
    /// Trials counted (spread minimisations' solves + joint trials).
    pub solves: u64,
    pub trials_run: u64,
}

struct Ctx<'a> {
    centre: &'a [i32],
    start_list: &'a [Offset],
    policy: &'a SeedMinPolicy,
    label: String,
    seq: usize,
    trials: Vec<TrialLog>,
    trials_run: u64,
    solves: u64,
    spread_records: Vec<Value>,
}

impl Ctx<'_> {
    fn trial(&mut self, kind: &str, list: &[Offset], keys: &[Key], mode: TrialMode) -> Trial {
        self.seq += 1;
        Trial {
            label: format!("{}/{}/{}", self.label, self.seq, kind),
            centre: self.centre.to_vec(),
            list: list.to_vec(),
            keys: keys.to_vec(),
            mode,
            engine_minimise: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn log(
        &mut self,
        t: &Trial,
        kind: &str,
        removed: &[Offset],
        a: &[TrialAnswer],
        kept: Option<bool>,
        reason: String,
        counted: bool,
    ) {
        self.trials.push(TrialLog {
            label: t.label.clone(),
            digest: t.digest(),
            kind: kind.into(),
            removed: (removed.len() == 1).then(|| removed[0].clone()),
            removed_chunk: if removed.len() > 1 {
                removed.to_vec()
            } else {
                vec![]
            },
            keys: t.keys.len(),
            seeds: t.list.len(),
            answers: a.iter().map(|x| x.word()).collect(),
            kept,
            reason,
            counted,
        });
    }

    fn run(
        &mut self,
        solver: &mut dyn TrialSolver,
        trials: &[Trial],
    ) -> Result<Vec<Vec<TrialAnswer>>, SeedMinError> {
        self.trials_run += trials.len() as u64;
        run_batch(solver, trials)
    }
}

/// The spread keys of `domain` (indices into it).
fn spread_keys(domain: &[Key], policy: &SeedMinPolicy) -> Vec<usize> {
    let sp = &policy.joint.spread;
    let n = sp.count.min(domain.len()).max(1);
    match sp.method.as_str() {
        "explicit" => {
            let v: Vec<usize> = sp
                .keys
                .iter()
                .filter_map(|k| domain.iter().position(|d| d == k))
                .collect();
            if v.is_empty() {
                farthest(domain, n, &[])
            } else {
                v
            }
        }
        "first" => (0..n).collect(),
        _ => farthest(domain, n, &[]),
    }
}

fn l1(a: &[i32], b: &[i32]) -> i64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs() as i64).sum()
}

/// Farthest-point selection: `chosen` first (or the first key), then repeatedly the key farthest (L1) from the chosen
/// set; ties: earlier in the domain order.
fn farthest(domain: &[Key], n: usize, chosen: &[usize]) -> Vec<usize> {
    let mut out: Vec<usize> = chosen.to_vec();
    if out.is_empty() && !domain.is_empty() {
        out.push(0);
    }
    while out.len() < n.min(domain.len()) {
        let mut best: Option<(i64, usize)> = None;
        for (i, k) in domain.iter().enumerate() {
            if out.contains(&i) {
                continue;
            }
            let d = out.iter().map(|&j| l1(k, &domain[j])).min().unwrap_or(0);
            if best.is_none_or(|(bd, _)| d > bd) {
                best = Some((d, i));
            }
        }
        match best {
            Some((_, i)) => out.push(i),
            None => break,
        }
    }
    out
}

/// Per key: does `new` (answer under `after`) keep the key acceptably against `old` (under `before`)?
#[allow(clippy::too_many_arguments)]
fn key_ok(
    key: &Key,
    old: &RelationFacts,
    new: &TrialAnswer,
    before: &[Offset],
    after: &[Offset],
    centre: &[i32],
    policy: &SeedMinPolicy,
    system: &mut dyn SystemPoles,
) -> (bool, String) {
    let Some(n) = new.relation() else {
        return (false, new.word());
    };
    let v = judge_change(
        &[State { key, facts: old }],
        &[State { key, facts: n }],
        before,
        after,
        centre,
        policy,
        system,
    );
    (v.keep, v.reason)
}

/// Minimise `list` (centre-relative) JOINTLY over `keys` (interfaces/seedmin.md 5).
pub fn minimise_joint(
    keys: &[Key],
    centre: &[i32],
    list: &[Offset],
    policy: &SeedMinPolicy,
    solver: &mut dyn TrialSolver,
    system: &mut dyn SystemPoles,
) -> Result<JointMin, SeedMinError> {
    check_input(keys, centre, list)?;
    preflight(policy, solver)?;
    let ordered = row_ordered(list, centre, policy);
    let list = ordered.as_slice();
    let mut cx = Ctx {
        centre,
        start_list: list,
        policy,
        label: format!("seedmin/joint{}", key_label(centre)),
        seq: 0,
        trials: vec![],
        trials_run: 0,
        solves: 0,
        spread_records: vec![],
    };
    // 1. START at every key
    let t = cx.trial("start", list, keys, TrialMode::Solve);
    let a = cx.run(solver, std::slice::from_ref(&t))?.remove(0);
    cx.log(&t, "start", &[], &a, None, "start".into(), false);
    let mut start: BTreeMap<Key, RelationFacts> = BTreeMap::new();
    let mut served: Vec<Key> = vec![];
    let mut unserved: Vec<Key> = vec![];
    let mut undecided: Vec<(Key, String)> = vec![];
    for (k, x) in keys.iter().zip(&a) {
        match x {
            TrialAnswer::Relation(r) => {
                start.insert(k.clone(), (**r).clone());
                served.push(k.clone());
            }
            TrialAnswer::NoRelation | TrialAnswer::NotInDomain => unserved.push(k.clone()),
            other => undecided.push((k.clone(), other.word())),
        }
    }
    // 2-5. pieces
    let mut pieces: Vec<Piece> = vec![];
    let mut remaining = served.clone();
    while !remaining.is_empty() {
        if pieces.len() >= policy.joint.max_pieces || policy.mode == "off" {
            pieces.push(start_piece(&remaining, list, &start, pieces.len()));
            break;
        }
        let (piece, failing) =
            build_piece(&remaining, &start, pieces.len(), &mut cx, solver, system)?;
        pieces.push(piece);
        if failing.len() == remaining.len() {
            // no progress (cannot happen: a piece always takes a key); guard against a loop
            pieces.push(start_piece(&failing, list, &start, pieces.len()));
            break;
        }
        remaining = failing;
    }
    // rows at d0 (diagnostic; candidates only)
    if policy.rows_at_d0 {
        for p in pieces.iter_mut() {
            let label = format!("{}/piece{}", cx.label, p.label);
            let d0 = rows_at_d0_pass(
                &p.keys,
                centre,
                list,
                &p.list,
                Some(p.per_key.clone()),
                policy,
                solver,
                system,
                &label,
            )?;
            if let Some((l, f)) = d0.adopted {
                p.list = l;
                p.per_key = f;
            }
            p.rows_at_d0 = Some(d0.record);
        }
    }
    // 6. choice sets
    let labels: Vec<String> = pieces
        .iter()
        .enumerate()
        .map(|(i, p)| format!("seedmin:{i}:{}", &list_digest(&p.list)[..12]))
        .collect();
    let mut acceptable: BTreeMap<Key, BTreeSet<String>> = BTreeMap::new();
    for (i, p) in pieces.iter().enumerate() {
        for k in &p.keys {
            acceptable
                .entry(k.clone())
                .or_default()
                .insert(labels[i].clone());
        }
    }
    if policy.joint.choice_sets_complete && pieces.len() > 1 {
        let mut ts = vec![];
        let mut idx = vec![];
        for (i, p) in pieces.iter().enumerate() {
            let own: BTreeSet<&Key> = p.keys.iter().collect();
            let others: Vec<Key> = served
                .iter()
                .filter(|k| !own.contains(k))
                .cloned()
                .collect();
            if !others.is_empty() {
                ts.push(cx.trial("choice", &p.list, &others, TrialMode::Solve));
                idx.push(i);
            }
        }
        let ans = cx.run(solver, &ts)?;
        for ((t, a), i) in ts.iter().zip(&ans).zip(&idx) {
            cx.log(
                t,
                "choice",
                &[],
                a,
                None,
                format!("choice set of {}", labels[*i]),
                false,
            );
            for (k, x) in t.keys.iter().zip(a) {
                if x.relation().is_some() {
                    acceptable
                        .entry(k.clone())
                        .or_default()
                        .insert(labels[*i].clone());
                }
            }
        }
    }
    let choice_sets = json!({
        "signature": format!("seedmin:centre{}", key_label(centre)),
        "keys": keys.iter().map(|k| json!({"key": k,
            "acceptable": acceptable.get(k).map(|s| s.iter().cloned().collect::<Vec<_>>()).unwrap_or_default()}))
            .collect::<Vec<_>>()});
    let record = json!({
        "schema": "rustred.campaign.seedmin.joint.v1", "backend": policy.backend,
        "policy_digest": policy.digest(), "centre": centre, "keys": keys.len(),
        "row_order": policy.row_order, "pole_objective": policy.pole_objective,
        "start": {"seeds": list.len(), "list_sha256": list_digest(list), "served": served.len(),
                  "unserved": unserved, "undecided": undecided},
        "spread": cx.spread_records,
        "pieces": pieces.iter().zip(&labels).map(|(p, l)| json!({"label": l, "keys": p.keys.len(),
            "seeds": p.list.len(), "list_sha256": list_digest(&p.list), "how": p.how,
            "rhs_max": p.per_key.iter().map(|f| f.rhs_count).max(),
            "minimise": p.minimise, "domain_tested": p.domain_tested, "rows_at_d0": p.rows_at_d0}))
            .collect::<Vec<_>>(),
        "solves": cx.solves, "trials_run": cx.trials_run, "trials": cx.trials});
    for (p, l) in pieces.iter_mut().zip(&labels) {
        p.label = l.clone();
    }
    Ok(JointMin {
        pieces,
        unserved,
        undecided,
        choice_sets,
        record,
        solves: cx.solves,
        trials_run: cx.trials_run,
    })
}

/// A piece served by the START list (keys left after `max_pieces`, or mode off): certified at step 1, no new solve.
fn start_piece(
    keys: &[Key],
    list: &[Offset],
    start: &BTreeMap<Key, RelationFacts>,
    i: usize,
) -> Piece {
    Piece {
        label: format!("{i}"),
        keys: keys.to_vec(),
        list: list.to_vec(),
        per_key: keys.iter().map(|k| start[k].clone()).collect(),
        how: "start_list".into(),
        minimise: None,
        domain_tested: json!({"keys": keys.len(), "solved": keys.len(), "failed": [],
                              "by": "the start trial"}),
        rows_at_d0: None,
    }
}

/// Per-key answers of a confirmation and the failing keys with their reasons.
type Confirmation = (Vec<TrialAnswer>, Vec<(Key, String)>);

/// Confirm `cand` at `domain` against the start relations: (per-key answers, failing keys with reasons).
fn confirm(
    cx: &mut Ctx<'_>,
    solver: &mut dyn TrialSolver,
    system: &mut dyn SystemPoles,
    cand: &[Offset],
    domain: &[Key],
    start: &BTreeMap<Key, RelationFacts>,
    kind: &str,
) -> Result<Confirmation, SeedMinError> {
    let t = cx.trial(kind, cand, domain, TrialMode::Solve);
    let a = cx.run(solver, std::slice::from_ref(&t))?.remove(0);
    cx.solves += 1;
    let mut fail = vec![];
    for (k, x) in domain.iter().zip(&a) {
        let (ok, why) = key_ok(
            k,
            &start[k],
            x,
            cx.start_list,
            cand,
            cx.centre,
            cx.policy,
            system,
        );
        if !ok {
            fail.push((k.clone(), why));
        }
    }
    cx.log(
        &t,
        kind,
        &[],
        &a,
        Some(fail.is_empty()),
        format!("{} of {} keys fail", fail.len(), domain.len()),
        true,
    );
    Ok((a, fail))
}

/// One per-key minimisation from the start list at each of `ks` (lockstep), recorded; returns the lists.
fn minimise_at(
    cx: &mut Ctx<'_>,
    solver: &mut dyn TrialSolver,
    system: &mut dyn SystemPoles,
    ks: &[Key],
    start: &BTreeMap<Key, RelationFacts>,
) -> Result<Vec<Vec<Offset>>, SeedMinError> {
    let policy = cx.policy;
    let mut ms: Vec<KeyMachine<'_>> = ks
        .iter()
        .map(|k| {
            let a = TrialAnswer::Relation(Box::new(start[k].clone()));
            KeyMachine::new(
                k,
                cx.centre,
                cx.start_list,
                policy,
                format!("seedmin/key{}", key_label(k)),
                Some(&a),
            )
        })
        .collect();
    run_lockstep(&mut ms, solver, system)?;
    let mut out = vec![];
    for m in ms {
        let km: KeyMin = m.finish();
        cx.solves += km.solves;
        cx.trials_run += km.trials_run;
        cx.spread_records.push(json!({"key": km.key, "seeds_before": km.seeds_before,
            "seeds_after": km.list.len(), "rhs_before": km.start_relation.as_ref().map(|r| r.rhs_count),
            "rhs_after": km.relation.as_ref().map(|r| r.rhs_count), "support": km.support,
            "stopped": km.stopped, "solves": km.solves, "minimise": km.minimise, "trials": km.trials}));
        out.push(km.list);
    }
    Ok(out)
}

/// Build one piece on `domain` (every key has a start relation): returns the piece and the keys handed on.
fn build_piece(
    domain: &[Key],
    start: &BTreeMap<Key, RelationFacts>,
    index: usize,
    cx: &mut Ctx<'_>,
    solver: &mut dyn TrialSolver,
    system: &mut dyn SystemPoles,
) -> Result<(Piece, Vec<Key>), SeedMinError> {
    let policy = cx.policy;
    let start_list = cx.start_list;
    // 2. spread keys and the union
    let mut chosen = spread_keys(domain, policy);
    let sk: Vec<Key> = chosen.iter().map(|&i| domain[i].clone()).collect();
    let lists = minimise_at(cx, solver, system, &sk, start)?;
    let mut cand = union_in_order(start_list, &lists);
    // 3. confirmation at every key, augmentation, repair / split
    let (mut ans, mut fail) = confirm(cx, solver, system, &cand, domain, start, "confirm")?;
    let mut extra = 0;
    while !fail.is_empty() && extra < policy.joint.spread.max_extra {
        let failing: BTreeSet<&Key> = fail.iter().map(|(k, _)| k).collect();
        // the first failing key in farthest order from the chosen set, not minimised yet (a minimised key that still
        // fails under the union is left to repair / split: minimising it again gives the same list)
        let order = farthest(domain, domain.len(), &chosen);
        let Some(&next) = order
            .iter()
            .find(|&&i| failing.contains(&domain[i]) && !chosen.contains(&i))
        else {
            break;
        };
        chosen.push(next);
        let l = minimise_at(
            cx,
            solver,
            system,
            std::slice::from_ref(&domain[next]),
            start,
        )?;
        cand = union_in_order(start_list, &[cand.clone(), l[0].clone()]);
        (ans, fail) = confirm(cx, solver, system, &cand, domain, start, "confirm")?;
        extra += 1;
    }
    if !fail.is_empty() && policy.joint.on_candidate_failure == "repair_union" {
        let mut parts = vec![cand.clone()];
        for (k, _) in &fail {
            parts.push(
                start[k]
                    .seeds_used
                    .clone()
                    .unwrap_or_else(|| start_list.to_vec()),
            );
        }
        let rep = union_in_order(start_list, &parts);
        let (a2, f2) = confirm(cx, solver, system, &rep, domain, start, "repair")?;
        if f2.len() < fail.len() {
            cand = rep;
            ans = a2;
            fail = f2;
        }
    }
    let failing: BTreeSet<Key> = fail.iter().map(|(k, _)| k.clone()).collect();
    let keys: Vec<Key> = domain
        .iter()
        .filter(|k| !failing.contains(*k))
        .cloned()
        .collect();
    if keys.is_empty() {
        // no candidate serves a key: the start list keeps the whole domain
        let p = start_piece(domain, start_list, start, index);
        return Ok((p, vec![]));
    }
    let mut cur: Vec<RelationFacts> = domain
        .iter()
        .zip(&ans)
        .filter(|(k, _)| !failing.contains(*k))
        .map(|(_, a)| a.relation().cloned().expect("a passing key has a relation"))
        .collect();
    // 4. joint greedy
    let mut stopped = "done".to_string();
    let mut solves_piece: u64 = 0;
    if policy.mode == "greedy" {
        let order: Vec<Offset> = drop_order(&cand, cx.centre, policy)
            .into_iter()
            .map(|i| cand[i].clone())
            .collect();
        let mut tried: BTreeSet<Offset> = BTreeSet::new();
        let mut witness: VecDeque<Key> = VecDeque::new();
        let mut rr = 0usize;
        // the joint strategy is its own (spec data): auto = halving above the size threshold
        let halving = match policy.joint.drop_chunks.as_str() {
            "halving" => true,
            "single" => false,
            _ => keys.len() > policy.joint.halving_above_keys,
        };
        let mut chunk = 0usize;
        // a refused removal set: a single seed is kept for good; a chunk halves (halving mode)
        let refuse = |set: &[Offset], tried: &mut BTreeSet<Offset>, chunk: &mut usize| {
            if set.len() == 1 {
                tried.insert(set[0].clone());
            } else {
                *chunk = (set.len() / policy.halving_factor).max(1);
            }
        };
        'outer: loop {
            let curset: BTreeSet<&Offset> = cand.iter().collect();
            let remaining: Vec<Offset> = order
                .iter()
                .filter(|s| curset.contains(s) && !tried.contains(*s))
                .cloned()
                .collect();
            if remaining.is_empty() || cand.len() <= 1 {
                break;
            }
            let left = policy.joint.max_solves.saturating_sub(solves_piece);
            if left == 0 {
                stopped = "max_solves".into();
                break;
            }
            // the screen subset of this batch: witnesses first, then round robin
            let full = keys.len() <= policy.joint.screen_keys;
            let screen: Vec<Key> = if full {
                keys.clone()
            } else {
                let mut s: Vec<Key> = witness
                    .iter()
                    .filter(|k| keys.contains(k))
                    .take(policy.joint.screen_keys)
                    .cloned()
                    .collect();
                while s.len() < policy.joint.screen_keys {
                    let k = keys[rr % keys.len()].clone();
                    rr += 1;
                    if !s.contains(&k) {
                        s.push(k);
                    }
                }
                s
            };
            // removal sets of this batch: single seeds (speculative, first kept wins) or ONE adaptive chunk
            let batch: Vec<Vec<Offset>> = if halving {
                if chunk == 0 {
                    chunk = if policy.chunk_start > 0 {
                        policy.chunk_start
                    } else {
                        (remaining.len() / policy.halving_factor).max(1)
                    };
                }
                let c = chunk.min(remaining.len()).min(cand.len() - 1).max(1);
                vec![remaining.into_iter().take(c).collect()]
            } else {
                let k = (policy.parallel_trials as u64).min(left) as usize;
                remaining.into_iter().take(k).map(|s| vec![s]).collect()
            };
            let ts: Vec<Trial> = batch
                .iter()
                .map(|s| cx.trial("screen", &without_all(&cand, s), &screen, TrialMode::Solve))
                .collect();
            let ans_b = cx.run(solver, &ts)?;
            for (bi, (t, a)) in ts.iter().zip(&ans_b).enumerate() {
                let s = &batch[bi];
                if solves_piece >= policy.joint.max_solves {
                    stopped = "max_solves".into();
                    break;
                }
                solves_piece += 1;
                cx.solves += 1;
                let after = &t.list;
                // screen check per key against the current relation of that key
                let mut bad: Option<(Key, String)> = None;
                for (k, x) in t.keys.iter().zip(a) {
                    let pos = keys
                        .iter()
                        .position(|y| y == k)
                        .expect("screen key in piece");
                    let (ok, why) =
                        key_ok(k, &cur[pos], x, &cand, after, cx.centre, policy, system);
                    if !ok {
                        bad = Some((k.clone(), why));
                        break;
                    }
                }
                if let Some((wk, why)) = bad {
                    refuse(s, &mut tried, &mut chunk);
                    witness.retain(|x| x != &wk);
                    witness.push_front(wk);
                    witness.truncate(policy.joint.screen_keys);
                    cx.log(
                        t,
                        "screen",
                        s,
                        a,
                        Some(false),
                        format!("screen: {why}"),
                        true,
                    );
                    continue;
                }
                // confirmation on EVERY key of the piece (never skipped; the screen is it when it covers the piece)
                let (conf_t, conf_a): (Trial, Vec<TrialAnswer>) = if full {
                    (t.clone(), a.clone())
                } else {
                    cx.log(t, "screen", s, a, None, "screen passed".into(), true);
                    if solves_piece >= policy.joint.max_solves {
                        stopped = "max_solves".into();
                        break;
                    }
                    let ct = cx.trial("confirm", after, &keys, TrialMode::Solve);
                    let ca = cx.run(solver, std::slice::from_ref(&ct))?.remove(0);
                    solves_piece += 1;
                    cx.solves += 1;
                    (ct, ca)
                };
                let fail_k = keys
                    .iter()
                    .zip(&conf_a)
                    .find(|(_, x)| x.relation().is_none())
                    .map(|(k, x)| (k.clone(), x.word()));
                let verdict = if let Some((fk, w)) = fail_k {
                    witness.retain(|x| x != &fk);
                    witness.push_front(fk.clone());
                    witness.truncate(policy.joint.screen_keys);
                    (false, format!("fails at {fk:?}: {w}"))
                } else {
                    let new: Vec<RelationFacts> = conf_a
                        .iter()
                        .map(|x| x.relation().cloned().expect("checked"))
                        .collect();
                    let olds: Vec<State<'_>> = keys
                        .iter()
                        .zip(&cur)
                        .map(|(k, f)| State { key: k, facts: f })
                        .collect();
                    let news: Vec<State<'_>> = keys
                        .iter()
                        .zip(&new)
                        .map(|(k, f)| State { key: k, facts: f })
                        .collect();
                    let v = judge_change(&olds, &news, &cand, after, cx.centre, policy, system);
                    (v.keep, v.reason)
                };
                cx.log(
                    &conf_t,
                    "confirm",
                    s,
                    &conf_a,
                    Some(verdict.0),
                    verdict.1.clone(),
                    true,
                );
                if verdict.0 {
                    cand = after.clone();
                    cur = conf_a
                        .iter()
                        .map(|x| x.relation().cloned().expect("checked"))
                        .collect();
                    if halving {
                        chunk = s.len() * policy.halving_factor;
                    }
                    for (t2, (a2, s2)) in ts.iter().zip(ans_b.iter().zip(&batch)).skip(bi + 1) {
                        cx.log(
                            t2,
                            "screen",
                            s2,
                            a2,
                            None,
                            "speculative: after the first kept drop".into(),
                            false,
                        );
                    }
                    continue 'outer;
                }
                refuse(s, &mut tried, &mut chunk);
            }
            if stopped == "max_solves" {
                break;
            }
        }
    }
    // 5. final verification at every key of the piece
    let (va, vfail) = {
        let t = cx.trial("verify", &cand, &keys, TrialMode::Solve);
        let a = cx.run(solver, std::slice::from_ref(&t))?.remove(0);
        let bad: Vec<Key> = keys
            .iter()
            .zip(&a)
            .filter(|(_, x)| x.relation().is_none())
            .map(|(k, _)| k.clone())
            .collect();
        cx.log(
            &t,
            "verify",
            &[],
            &a,
            Some(bad.is_empty()),
            format!(
                "final verification: {} of {} keys certified",
                keys.len() - bad.len(),
                keys.len()
            ),
            false,
        );
        (a, bad)
    };
    let mut handed: Vec<Key> = domain
        .iter()
        .filter(|k| failing.contains(*k))
        .cloned()
        .collect();
    handed.extend(vfail.iter().cloned());
    let keep_keys: Vec<Key> = keys
        .iter()
        .filter(|k| !vfail.contains(k))
        .cloned()
        .collect();
    let per_key: Vec<RelationFacts> = keys
        .iter()
        .zip(&va)
        .filter(|(k, _)| !vfail.contains(k))
        .map(|(_, a)| a.relation().cloned().expect("verified"))
        .collect();
    let digests: String = keep_keys
        .iter()
        .map(|k| start[k].relation_digest.clone())
        .collect::<Vec<_>>()
        .join("\n");
    let minimise = (policy.mode != "off").then(|| {
        json!({"mode": policy.mode, "measure": policy.measure, "order": policy.order,
            "solves": solves_piece, "stopped": stopped, "seeds_before": start_list.len(),
            "seeds_after": cand.len(),
            "rhs_before": keep_keys.iter().map(|k| start[k].rhs_count).max().unwrap_or(0),
            "rhs_after": per_key.iter().map(|f| f.rhs_count).max().unwrap_or(0),
            "original_relation_digest": sha256_hex(digests.as_bytes())})
    });
    let n_regular = per_key
        .iter()
        .filter(|f| f.pole == PoleOrder::regular())
        .count();
    let piece = Piece {
        label: format!("{index}"),
        keys: keep_keys.clone(),
        list: cand,
        per_key,
        how: "joint".into(),
        minimise,
        domain_tested: json!({"keys": keep_keys.len(), "solved": keep_keys.len(),
            // planfix S1: what the list is at its own keys (the only keys the pole objective judged)
            "regular": n_regular, "failed": [],
            "handed_on": handed.len(), "handed_on_reasons": fail.iter().map(|(k, w)| json!([k, w])).collect::<Vec<_>>(),
            "verification_failed": vfail}),
        rows_at_d0: None,
    };
    Ok((piece, handed))
}

/// The diagnostic rows-at-d0 pass.
pub(crate) struct D0Pass {
    pub adopted: Option<(Vec<Offset>, Vec<RelationFacts>)>,
    pub record: Value,
}

/// TRICKS 14 (c) / 16: a greedy pass whose trials run in PROBE mode at d0 (a drop is kept iff every key answers
/// EXISTS_AT_D0), from the start list; the list found is a CANDIDATE: solved and proved exactly in normal mode at every
/// key, then compared with the normal-mode result by the objective. Its failure is NO VERDICT: nothing is pruned,
/// nothing ends.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rows_at_d0_pass(
    keys: &[Key],
    centre: &[i32],
    start_list: &[Offset],
    normal_list: &[Offset],
    normal_facts: Option<Vec<RelationFacts>>,
    policy: &SeedMinPolicy,
    solver: &mut dyn TrialSolver,
    system: &mut dyn SystemPoles,
    label: &str,
) -> Result<D0Pass, SeedMinError> {
    let mode = TrialMode::Probe {
        d0: policy.d0.clone(),
    };
    let exists = |a: &[TrialAnswer]| {
        a.iter()
            .all(|x| matches!(x, TrialAnswer::Probe { verdict } if verdict == "EXISTS_AT_D0"))
    };
    let mut seq = 0usize;
    let mut mk = |kind: &str, list: &[Offset], m: TrialMode| {
        seq += 1;
        Trial {
            label: format!("{label}/d0/{seq}/{kind}"),
            centre: centre.to_vec(),
            list: list.to_vec(),
            keys: keys.to_vec(),
            mode: m,
            engine_minimise: None,
        }
    };
    let verdict_text = "none in either direction (TRICKS 16 / 17): rows at d0 prove nothing about the delivered \
                        relation; a success is only a candidate (accepted by pole_order_of on its own delivered relation), a \
                        failure never prunes a seed set, never ends a search, never marks a key";
    let mut probe_trials = 0u64;
    let t = mk("probe_start", start_list, mode.clone());
    let a = run_batch(solver, std::slice::from_ref(&t))?.remove(0);
    probe_trials += 1;
    if !exists(&a) {
        return Ok(D0Pass {
            adopted: None,
            record: json!({"outcome": "no_candidate", "verdict": verdict_text, "probe_trials": probe_trials,
                "exact_trials": 0, "start_answers": a.iter().map(|x| x.word()).collect::<Vec<_>>()}),
        });
    }
    let mut cur = start_list.to_vec();
    let order: Vec<Offset> = drop_order(&cur, centre, policy)
        .into_iter()
        .map(|i| cur[i].clone())
        .collect();
    let mut tried: BTreeSet<Offset> = BTreeSet::new();
    let mut solves = 0u64;
    loop {
        let set: BTreeSet<&Offset> = cur.iter().collect();
        let rem: Vec<Offset> = order
            .iter()
            .filter(|s| set.contains(s) && !tried.contains(*s))
            .cloned()
            .collect();
        let left = policy.max_solves.saturating_sub(solves);
        if rem.is_empty() || cur.len() <= 1 || left == 0 {
            break;
        }
        let batch: Vec<Offset> = rem
            .into_iter()
            .take((policy.parallel_trials as u64).min(left) as usize)
            .collect();
        let ts: Vec<Trial> = batch
            .iter()
            .map(|s| mk("probe_drop", &without(&cur, s), mode.clone()))
            .collect();
        let ans = run_batch(solver, &ts)?;
        probe_trials += ts.len() as u64;
        let mut kept = false;
        for (s, a) in batch.iter().zip(&ans) {
            solves += 1;
            tried.insert(s.clone());
            if exists(a) {
                cur = without(&cur, s);
                kept = true;
                break;
            }
        }
        let _ = kept;
    }
    let t = mk("probe_exact", &cur, TrialMode::Solve);
    let a = run_batch(solver, std::slice::from_ref(&t))?.remove(0);
    let facts: Option<Vec<RelationFacts>> = a.iter().map(|x| x.relation().cloned()).collect();
    let Some(facts) = facts else {
        return Ok(D0Pass {
            adopted: None,
            record: json!({"outcome": "candidate_failed", "verdict": verdict_text, "probe_trials": probe_trials,
                "exact_trials": 1, "seeds": cur.len(), "list_sha256": list_digest(&cur),
                "exact_answers": a.iter().map(|x| x.word()).collect::<Vec<_>>()}),
        });
    };
    let (adopt, reason) = match &normal_facts {
        None => (true, "the normal pass has no relation".to_string()),
        Some(nf) => {
            let olds: Vec<State<'_>> = keys
                .iter()
                .zip(nf)
                .map(|(k, f)| State { key: k, facts: f })
                .collect();
            let news: Vec<State<'_>> = keys
                .iter()
                .zip(&facts)
                .map(|(k, f)| State { key: k, facts: f })
                .collect();
            let v = judge_change(&olds, &news, normal_list, &cur, centre, policy, system);
            (v.keep, v.reason)
        }
    };
    let record = json!({"outcome": if adopt { "candidate_accepted" } else { "candidate_not_better" },
        "verdict": verdict_text, "probe_trials": probe_trials, "exact_trials": 1, "seeds": cur.len(),
        "list_sha256": list_digest(&cur), "reason": reason,
        "rhs_max": facts.iter().map(|f| f.rhs_count).max()});
    Ok(D0Pass {
        adopted: adopt.then(|| (cur.clone(), facts)),
        record,
    })
}

/// audit r2-planners S1: what a piece's list does BEYOND the keys it was minimised on, compared key by key with a FULL
/// (start) list. The joint minimiser applies the pole objective (per-key Pareto, `judge_change`) only at the piece's
/// own keys; at further keys the list was never judged, so the record states exactly that and lists the keys where the
/// minimised list is WORSE than the full list (singular where the full list is regular, or a higher pole order). Since rule 21
/// (2026-11-19) this is a REPORT: the default objective puts terms first and the pole order last (a tie-break); only under
/// the explicit pole-first objective (the withdrawn "regular at d = 4 wins") should a consumer not take the list there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionReport {
    pub piece: String,
    pub seeds: usize,
    /// The piece's own keys (minimised + verified there; the objective applied there).
    pub own_keys: usize,
    /// Further keys tried (not the piece's).
    pub extra_keys: usize,
    /// Of the further keys: a relation / a regular relation (pole order 0) with the piece's list.
    pub extra_valid: usize,
    pub extra_regular: usize,
    /// The same with the full list (None: no comparison asked).
    pub full_valid: Option<usize>,
    pub full_regular: Option<usize>,
    /// Further keys where the piece's list is worse than the full list: (key, piece's pole, full list's pole).
    pub worse_than_full: Vec<(Key, PoleOrder, PoleOrder)>,
    /// Further keys where the piece's list gives no relation: (key, answer word).
    pub failed: Vec<(Key, String)>,
    /// What is claimed, in words.
    pub claim: String,
}

fn worse_pole(min: PoleOrder, full: PoleOrder) -> bool {
    match (min, full) {
        (PoleOrder::Order { order: a }, PoleOrder::Order { order: b }) => a > b,
        (PoleOrder::Singular, PoleOrder::Order { order: 0 }) => true,
        _ => false,
    }
}

/// Solve the piece's list (and, when given, the FULL list) at the `extra` keys (keys of the piece itself are skipped),
/// in trials of at most `chunk` keys, and report per [`ExtensionReport`]. Deterministic; one trial batch per list.
pub fn verify_beyond_piece(
    piece: &Piece,
    centre: &[i32],
    full_list: Option<&[Offset]>,
    extra: &[Key],
    chunk: usize,
    solver: &mut dyn TrialSolver,
) -> Result<ExtensionReport, SeedMinError> {
    let own: std::collections::BTreeSet<&Key> = piece.keys.iter().collect();
    let extra: Vec<Key> = extra.iter().filter(|k| !own.contains(k)).cloned().collect();
    let chunk = chunk.max(1);
    let mut solve = |what: &str, list: &[Offset]| -> Result<Vec<TrialAnswer>, SeedMinError> {
        let trials: Vec<Trial> = extra
            .chunks(chunk)
            .enumerate()
            .map(|(i, ks)| Trial {
                label: format!("seedmin/extension/{}/{what}/{i}", piece.label),
                centre: centre.to_vec(),
                list: list.to_vec(),
                keys: ks.to_vec(),
                mode: TrialMode::Solve,
                engine_minimise: None,
            })
            .collect();
        let ans = solver
            .solve(&trials)
            .map_err(|message| SeedMinError::Solver { message })?;
        let all: Vec<TrialAnswer> = ans.into_iter().flatten().collect();
        if all.len() != extra.len() {
            return Err(SeedMinError::Protocol {
                message: format!("{} answers for {} keys", all.len(), extra.len()),
            });
        }
        Ok(all)
    };
    let mine = solve("list", &piece.list)?;
    let full = match full_list {
        Some(l) => Some(solve("full", l)?),
        None => None,
    };
    let regular = |a: &TrialAnswer| a.relation().is_some_and(|f| f.pole == PoleOrder::regular());
    let extra_valid = mine.iter().filter(|a| a.relation().is_some()).count();
    let extra_regular = mine.iter().filter(|a| regular(a)).count();
    let mut worse = vec![];
    if let Some(fa) = &full {
        for ((k, m), f) in extra.iter().zip(&mine).zip(fa) {
            if let (Some(m), Some(f)) = (m.relation(), f.relation())
                && worse_pole(m.pole, f.pole)
            {
                worse.push((k.clone(), m.pole, f.pole));
            }
        }
    }
    let failed: Vec<(Key, String)> = extra
        .iter()
        .zip(&mine)
        .filter(|(_, a)| a.relation().is_none())
        .map(|(k, a)| (k.clone(), a.word()))
        .collect();
    let full_valid = full
        .as_ref()
        .map(|f| f.iter().filter(|a| a.relation().is_some()).count());
    let full_regular = full
        .as_ref()
        .map(|f| f.iter().filter(|a| regular(a)).count());
    let claim = format!(
        "pole objective applied at the {} piece keys only; at {} further keys: a relation at {}, regular at {}{}",
        piece.keys.len(),
        extra.len(),
        extra_valid,
        extra_regular,
        match (full_valid, full_regular) {
            (Some(v), Some(r)) => format!(
                "; the full list: a relation at {v}, regular at {r}; the piece's list is WORSE than the full list at {} \
                 key(s) (listed): not to be taken there under the pole objective",
                worse.len()
            ),
            _ => "; not compared with the full list".into(),
        }
    );
    Ok(ExtensionReport {
        piece: piece.label.clone(),
        seeds: piece.list.len(),
        own_keys: piece.keys.len(),
        extra_keys: extra.len(),
        extra_valid,
        extra_regular,
        full_valid,
        full_regular,
        worse_than_full: worse,
        failed,
        claim,
    })
}
