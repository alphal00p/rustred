//! Lane seedmin: pure tests of the minimiser with an in-process ORACLE CHILD (a key solves with a list iff the list
//! contains one of the key's required seed sets, the first in oracle order wins; seeds_used = that set). Arity 3, toy
//! data: nothing five-loop specific.

use std::collections::{BTreeMap, BTreeSet};

use rustred_campaign_core::seedmin::journal::JournaledSolver;
use rustred_campaign_core::seedmin::{
    EngineCaps, Key, Offset, PoleOrder, RelationFacts, SeedMinError, SeedMinPolicy,
    StubSystemPoles, SystemPoles, Trial, TrialAnswer, TrialMode, TrialSolver, minimise_joint,
    minimise_key, rule_candidate, sha256_hex,
};
use serde_json::{Value, json};

/// One way a key solves: the seeds it needs (centre-relative), the relation's size and pole order.
#[derive(Clone)]
struct Alt {
    needs: Vec<Offset>,
    rhs: u64,
    pole: PoleOrder,
    /// Probe answer of a list containing this alternative (rows at d0).
    probe: &'static str,
}

#[derive(Clone, Default)]
struct Oracle {
    alts: BTreeMap<Key, Vec<Alt>>,
    report_used: bool,
    caps: EngineCaps,
    /// Error after this many trials (an interrupted run).
    fail_after: Option<usize>,
    solved: usize,
    log: Vec<String>,
}

impl Oracle {
    fn answer(&self, t: &Trial, k: &Key) -> TrialAnswer {
        let have: BTreeSet<&Offset> = t.list.iter().collect();
        let Some(alts) = self.alts.get(k) else {
            return TrialAnswer::NotInDomain;
        };
        for (i, a) in alts.iter().enumerate() {
            if a.needs.iter().all(|o| have.contains(o)) {
                if let TrialMode::Probe { .. } = t.mode {
                    return TrialAnswer::Probe {
                        verdict: a.probe.into(),
                    };
                }
                return TrialAnswer::Relation(Box::new(RelationFacts {
                    relation_digest: sha256_hex(format!("{k:?}/{i}").as_bytes()),
                    rhs_count: a.rhs,
                    rhs_sub_count: Some(a.rhs / 2),
                    bytes: Some(a.rhs * 100),
                    trace_rows: Some(a.needs.len() as u64 * 7),
                    seeds_used: self.report_used.then(|| a.needs.clone()),
                    pole: a.pole,
                    minimise: None,
                }));
            }
        }
        if let TrialMode::Probe { .. } = t.mode {
            return TrialAnswer::Probe {
                verdict: "NO_RELATION_AT_D0".into(),
            };
        }
        TrialAnswer::NoRelation
    }
}

impl TrialSolver for Oracle {
    fn solve(&mut self, trials: &[Trial]) -> Result<Vec<Vec<TrialAnswer>>, String> {
        let mut out = vec![];
        for t in trials {
            if let Some(n) = self.fail_after
                && self.solved >= n
            {
                return Err("interrupted".into());
            }
            self.solved += 1;
            self.log.push(t.label.clone());
            out.push(t.keys.iter().map(|k| self.answer(t, k)).collect());
        }
        Ok(out)
    }
    fn capabilities(&self) -> EngineCaps {
        self.caps.clone()
    }
}

fn alt(needs: &[[i32; 3]], rhs: u64) -> Alt {
    Alt {
        needs: needs.iter().map(|o| o.to_vec()).collect(),
        rhs,
        pole: PoleOrder::regular(),
        probe: "EXISTS_AT_D0",
    }
}

/// The shell list of a toy: every offset with entries in -1..=1 (27), generation order.
fn shell() -> Vec<Offset> {
    let mut v = vec![];
    for a in -1..=1 {
        for b in -1..=1 {
            for c in -1..=1 {
                v.push(vec![a, b, c]);
            }
        }
    }
    v
}

const C0: [i32; 3] = [0, 0, 0];

#[test]
fn support_step_restricts_to_seeds_used_then_greedy_keeps_the_essential_seeds() {
    let k: Key = vec![3, 2, -1];
    let mut o = Oracle {
        report_used: true,
        ..Default::default()
    };
    o.alts.insert(
        k.clone(),
        vec![alt(&[[0, 0, 0], [1, 0, 0], [0, 1, -1]], 40)],
    );
    let p = SeedMinPolicy::default();
    let km = minimise_key(&k, &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(km.support, "applied");
    assert_eq!(km.list, vec![vec![0, 0, 0], vec![0, 1, -1], vec![1, 0, 0]]);
    // 1 support solve + 3 drop trials (each needed seed refused); no resupport (nothing kept)
    assert_eq!(km.solves, 4);
    assert_eq!(km.stopped, "done");
    let m = km.minimise.as_ref().unwrap();
    assert_eq!(m["seeds_before"], 27);
    assert_eq!(m["seeds_after"], 3);
    assert_eq!(m["mode"], "greedy");
    assert_eq!(m["order"], "distance");
    assert_eq!(m["original_relation_digest"].as_str().unwrap().len(), 64);
    // without seeds_used: support unavailable, the greedy pass alone gets the same list
    o.report_used = false;
    let km2 = minimise_key(&k, &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(km2.support, "unavailable");
    assert_eq!(km2.list, km.list);
    assert_eq!(km2.solves, 27); // every seed offered once (24 kept, the 3 needed refused)
}

#[test]
fn greedy_offers_seeds_in_distance_order_outermost_first() {
    let k: Key = vec![2, 2, -1];
    let mut o = Oracle::default();
    o.alts.insert(k.clone(), vec![alt(&[[0, 0, 0]], 5)]);
    let p = SeedMinPolicy {
        parallel_trials: 1,
        ..SeedMinPolicy::default()
    };
    let km = minimise_key(&k, &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    let removed: Vec<Offset> = km
        .trials
        .iter()
        .filter(|t| t.kind == "drop")
        .filter_map(|t| t.removed.clone())
        .collect();
    let dist = |o: &Offset| o.iter().map(|x| x.unsigned_abs()).sum::<u32>();
    // non-increasing distance; ties: later in the list first
    for w in removed.windows(2) {
        assert!(dist(&w[0]) >= dist(&w[1]), "{:?} before {:?}", w[0], w[1]);
        if dist(&w[0]) == dist(&w[1]) {
            let s = shell();
            let i0 = s.iter().position(|x| *x == w[0]).unwrap();
            let i1 = s.iter().position(|x| *x == w[1]).unwrap();
            assert!(i0 > i1);
        }
    }
    assert_eq!(removed.first().unwrap(), &vec![1, 1, 1]);
    assert_eq!(km.list, vec![vec![0, 0, 0]]);
    // speculative batches give the same result and the same sequential count
    let mut q = p.clone();
    q.parallel_trials = 5;
    let km5 = minimise_key(&k, &C0, &shell(), &q, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(km5.result_record(), km.result_record());
    assert_eq!(km5.solves, km.solves);
}

#[test]
fn a_drop_that_worsens_the_pole_order_is_refused_and_pole_objective_off_is_plain_size() {
    let k: Key = vec![2, 3, 0];
    let mut o = Oracle::default();
    let mut sing = alt(&[[0, 0, 0]], 3);
    sing.pole = PoleOrder::Order { order: 1 };
    // the regular relation needs three seeds; the singular one only the centre (first match in oracle order wins)
    o.alts.insert(
        k.clone(),
        vec![alt(&[[0, 0, 0], [1, 0, 0], [0, 1, 0]], 10), sing],
    );
    let p = SeedMinPolicy::default();
    let km = minimise_key(&k, &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(km.relation.as_ref().unwrap().pole, PoleOrder::regular());
    assert_eq!(
        km.list.len(),
        3,
        "the rows that make the relation regular are kept"
    );
    let mut off = p.clone();
    off.pole_objective = "off".into();
    let km2 = minimise_key(&k, &C0, &shell(), &off, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(km2.list, vec![vec![0, 0, 0]], "plain size minimisation");
    assert_eq!(
        km2.relation.as_ref().unwrap().pole,
        PoleOrder::Order { order: 1 }
    );
    // undecided pole orders: keep_seed refuses every drop, drop proceeds
    let mut und = alt(&[[0, 0, 0]], 3);
    und.pole = PoleOrder::Undecided;
    let mut o2 = Oracle::default();
    o2.alts.insert(k.clone(), vec![und]);
    let km3 = minimise_key(&k, &C0, &shell(), &p, &mut o2, &mut StubSystemPoles).unwrap();
    assert_eq!(km3.list.len(), 27);
    let mut drop = p.clone();
    drop.on_undecided_regularity = "drop".into();
    let km4 = minimise_key(&k, &C0, &shell(), &drop, &mut o2, &mut StubSystemPoles).unwrap();
    assert_eq!(km4.list, vec![vec![0, 0, 0]]);
}

/// A system judge that says "worse" whenever the after-list lacks a given seed.
struct NeedsSeed(Offset);
impl SystemPoles for NeedsSeed {
    fn available(&self) -> bool {
        true
    }
    fn keeps_regularity(
        &mut self,
        _b: &[Offset],
        a: &[Offset],
        _k: &[i32],
        _d0: &str,
    ) -> Option<bool> {
        Some(a.contains(&self.0))
    }
    fn name(&self) -> String {
        "needs-seed".into()
    }
}

#[test]
fn the_system_judge_vetoes_drops_when_available() {
    let k: Key = vec![2, 2, 0];
    let mut o = Oracle::default();
    o.alts.insert(k.clone(), vec![alt(&[[0, 0, 0]], 5)]);
    let p = SeedMinPolicy::default();
    let km = minimise_key(&k, &C0, &shell(), &p, &mut o, &mut NeedsSeed(vec![1, 1, 1])).unwrap();
    assert!(km.list.contains(&vec![1, 1, 1]));
    assert!(km.list.contains(&vec![0, 0, 0]));
    assert_eq!(km.list.len(), 2);
}

fn two_key_oracle() -> (Oracle, Key, Key) {
    // A needs {a}, B needs {b}: a list minimised at A alone OVERFITS (fails at B)
    let ka: Key = vec![2, 1, 0];
    let kb: Key = vec![3, 1, 0];
    let mut o = Oracle {
        report_used: true,
        ..Default::default()
    };
    o.alts
        .insert(ka.clone(), vec![alt(&[[0, 0, 0], [1, 0, 0]], 8)]);
    o.alts
        .insert(kb.clone(), vec![alt(&[[0, 0, 0], [0, 1, -1]], 9)]);
    (o, ka, kb)
}

#[test]
fn an_overfit_list_is_rejected_by_the_every_key_confirmation() {
    let (mut o, ka, kb) = two_key_oracle();
    let mut p = SeedMinPolicy::default();
    p.joint.spread.count = 1; // only A is minimised first: its list overfits
    let per_key = minimise_key(&ka, &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    let t = Trial {
        label: "check".into(),
        centre: C0.to_vec(),
        list: per_key.list.clone(),
        keys: vec![kb.clone()],
        mode: TrialMode::Solve,
        engine_minimise: None,
    };
    assert_eq!(
        o.solve(&[t]).unwrap()[0][0],
        TrialAnswer::NoRelation,
        "overfit at B"
    );
    let jm = minimise_joint(
        &[ka.clone(), kb.clone()],
        &C0,
        &shell(),
        &p,
        &mut o,
        &mut StubSystemPoles,
    )
    .unwrap();
    assert_eq!(jm.pieces.len(), 1);
    let pc = &jm.pieces[0];
    assert_eq!(pc.keys, vec![ka.clone(), kb.clone()]);
    assert_eq!(pc.list, vec![vec![0, 0, 0], vec![0, 1, -1], vec![1, 0, 0]]);
    // the first confirmation (A's list at every key) failed at B; B was minimised (augmentation) and joined
    let kinds: Vec<(String, Option<bool>)> = jm.record["trials"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["kind"].as_str().unwrap().to_string(), t["kept"].as_bool()))
        .collect();
    assert_eq!(kinds[1], ("confirm".to_string(), Some(false)));
    assert_eq!(kinds[2], ("confirm".to_string(), Some(true)));
    assert!(kinds.iter().any(|(k, _)| k == "verify"));
    assert_eq!(jm.record["spread"].as_array().unwrap().len(), 2);
    assert_eq!(pc.domain_tested["solved"], 2);
}

#[test]
fn a_failing_key_splits_the_domain_into_pieces_with_choice_sets() {
    let (mut o, ka, kb) = two_key_oracle();
    let mut p = SeedMinPolicy::default();
    p.joint.spread.count = 1;
    p.joint.spread.max_extra = 0;
    p.joint.on_candidate_failure = "split".into();
    let jm = minimise_joint(
        &[ka.clone(), kb.clone()],
        &C0,
        &shell(),
        &p,
        &mut o,
        &mut StubSystemPoles,
    )
    .unwrap();
    assert_eq!(jm.pieces.len(), 2);
    assert_eq!(jm.pieces[0].keys, vec![ka.clone()]);
    assert_eq!(jm.pieces[0].list, vec![vec![0, 0, 0], vec![1, 0, 0]]);
    assert_eq!(jm.pieces[1].keys, vec![kb.clone()]);
    assert_eq!(jm.pieces[1].list, vec![vec![0, 0, 0], vec![0, 1, -1]]);
    // choice sets in lane regions' form: each key accepts exactly its own piece's list
    let cs = &jm.choice_sets;
    let keys = cs["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 2);
    for (k, x) in keys.iter().zip([&jm.pieces[0], &jm.pieces[1]]) {
        assert_eq!(k["acceptable"].as_array().unwrap(), &vec![json!(x.label)]);
    }
    // the repair_union default instead keeps ONE piece
    let mut q = p.clone();
    q.joint.on_candidate_failure = "repair_union".into();
    let jm2 = minimise_joint(
        &[ka.clone(), kb.clone()],
        &C0,
        &shell(),
        &q,
        &mut o,
        &mut StubSystemPoles,
    )
    .unwrap();
    assert_eq!(jm2.pieces.len(), 1);
    assert_eq!(jm2.pieces[0].keys.len(), 2);
}

#[test]
fn unserved_keys_stay_outside_every_piece_and_max_pieces_keeps_the_start_list() {
    let (mut o, ka, kb) = two_key_oracle();
    let kc: Key = vec![4, 1, 0]; // no alternative: not in domain
    let kd: Key = vec![5, 1, 0];
    o.alts.insert(kd.clone(), vec![alt(&[[5, 5, 5]], 1)]); // needs a seed outside the list: no relation
    let mut p = SeedMinPolicy::default();
    p.joint.spread.count = 1;
    p.joint.spread.max_extra = 0;
    p.joint.on_candidate_failure = "split".into();
    p.joint.max_pieces = 1;
    let keys = vec![ka.clone(), kb.clone(), kc.clone(), kd.clone()];
    let jm = minimise_joint(&keys, &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(jm.unserved, vec![kc.clone(), kd.clone()]);
    assert_eq!(jm.pieces.len(), 2);
    assert_eq!(jm.pieces[1].how, "start_list");
    assert_eq!(jm.pieces[1].keys, vec![kb.clone()]);
    assert_eq!(jm.pieces[1].list.len(), 27);
    let cs = jm.choice_sets["keys"].as_array().unwrap();
    assert!(cs[2]["acceptable"].as_array().unwrap().is_empty());
    // the size candidate of piece 0 judged on an unserved sub-face {A, B}: B is not in piece 0
    let rc = rule_candidate(&jm.pieces[0], &[ka.clone(), kb.clone()]);
    assert_eq!(rc.keys.len(), 1);
    assert_eq!(rc.not_in_piece, 1);
    assert_eq!(rc.measures["rhs_terms"], 8);
}

#[test]
fn joint_greedy_drops_only_what_every_key_allows_screened_and_confirmed() {
    // three keys share the centre; each needs one private seed; a fourth seed is needed by nobody
    let ks: Vec<Key> = vec![
        vec![2, 1, 0],
        vec![3, 1, 0],
        vec![4, 1, 0],
        vec![5, 1, 0],
        vec![6, 1, 0],
    ];
    let mut o = Oracle::default();
    let private = [[1, 0, 0], [0, 1, 0], [0, 0, -1], [1, 1, 0], [1, 0, -1]];
    for (k, pr) in ks.iter().zip(private) {
        o.alts.insert(k.clone(), vec![alt(&[[0, 0, 0], pr], 7)]);
    }
    let mut p = SeedMinPolicy::default();
    p.joint.screen_keys = 2;
    let jm = minimise_joint(&ks, &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(jm.pieces.len(), 1);
    let got: BTreeSet<Offset> = jm.pieces[0].list.iter().cloned().collect();
    let want: BTreeSet<Offset> = private
        .iter()
        .map(|x| x.to_vec())
        .chain([vec![0, 0, 0]])
        .collect();
    assert_eq!(got, want);
    // every KEPT drop has a confirmation on all five keys
    for t in jm.record["trials"].as_array().unwrap() {
        if t["kind"] == "confirm" && t["kept"] == true && !t["removed"].is_null() {
            assert_eq!(t["keys"], 5);
        }
        if t["kind"] == "screen" {
            assert_eq!(t["keys"], 2);
        }
    }
    // the row order of the start list is kept
    let pos = |o: &Offset| shell().iter().position(|x| x == o).unwrap();
    let l = &jm.pieces[0].list;
    assert!(l.windows(2).all(|w| pos(&w[0]) < pos(&w[1])));
}

#[test]
fn resume_from_the_journal_gives_the_same_record_at_every_cut() {
    let (o0, ka, kb) = two_key_oracle();
    let mut p = SeedMinPolicy::default();
    p.joint.spread.count = 1;
    let keys = vec![ka.clone(), kb.clone()];
    let full = {
        let mut o = o0.clone();
        minimise_joint(&keys, &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap()
    };
    // distinct trials of an uninterrupted journaled run (the journal answers a repeated trial once)
    let total = {
        let mut js = JournaledSolver::new(o0.clone());
        let r = minimise_joint(&keys, &C0, &shell(), &p, &mut js, &mut StubSystemPoles).unwrap();
        assert_eq!(r.record, full.record);
        js.misses as usize
    };
    assert!(total > 6);
    for cut in [1, 3, total / 2, total - 1] {
        let mut o = o0.clone();
        o.fail_after = Some(cut);
        let mut js = JournaledSolver::new(o);
        let e =
            minimise_joint(&keys, &C0, &shell(), &p, &mut js, &mut StubSystemPoles).unwrap_err();
        assert!(matches!(e, SeedMinError::Solver { .. }));
        // a batch interrupted in its middle journals nothing of that batch
        let lines = js.take_new_lines();
        assert!(lines.len() <= cut);
        let mut js2 =
            JournaledSolver::from_lines(o0.clone(), lines.iter().map(|s| s.as_str())).unwrap();
        let again =
            minimise_joint(&keys, &C0, &shell(), &p, &mut js2, &mut StubSystemPoles).unwrap();
        assert_eq!(again.record, full.record, "cut {cut}");
        assert_eq!(js2.misses as usize + lines.len(), total, "cut {cut}");
    }
}

/// A fake ENGINE child: on a trial carrying `elimination.minimise` it runs the minimisation itself (the emulation on
/// its own oracle) and answers the contract's relation record with the `minimise` member (+ the proposed `seeds`).
struct FakeEngine {
    oracle: Oracle,
}

impl TrialSolver for FakeEngine {
    fn solve(&mut self, trials: &[Trial]) -> Result<Vec<Vec<TrialAnswer>>, String> {
        let mut out = vec![];
        for t in trials {
            let Some(m) = &t.engine_minimise else {
                out.extend(self.oracle.solve(std::slice::from_ref(t))?);
                continue;
            };
            let p = SeedMinPolicy {
                mode: m["mode"].as_str().unwrap().into(),
                order: m["order"].as_str().unwrap().into(),
                max_solves: m["max_solves"].as_u64().unwrap(),
                measure: m["measure"].as_str().unwrap().into(),
                ..SeedMinPolicy::default()
            };
            let mut v = vec![];
            for k in &t.keys {
                let km = minimise_key(
                    k,
                    &t.centre,
                    &t.list,
                    &p,
                    &mut self.oracle,
                    &mut StubSystemPoles,
                )
                .map_err(|e| e.to_string())?;
                match km.relation {
                    Some(mut r) => {
                        let mut mm: Value = km.minimise.clone().unwrap();
                        let seeds: Vec<Offset> = km
                            .list
                            .iter()
                            .map(|o| o.iter().zip(&t.centre).map(|(a, b)| a + b).collect())
                            .collect();
                        mm["seeds"] = json!(seeds);
                        r.minimise = Some(mm);
                        v.push(TrialAnswer::Relation(Box::new(r)));
                    }
                    None => v.push(TrialAnswer::NoRelation),
                }
            }
            out.push(v);
        }
        Ok(out)
    }
    fn capabilities(&self) -> EngineCaps {
        EngineCaps {
            minimise: true,
            features: vec!["seed_minimise".into()],
            ..Default::default()
        }
    }
}

#[test]
fn the_engine_verb_and_the_emulation_give_the_same_result_record() {
    let k: Key = vec![3, 2, -1];
    let centre = vec![-1, 0, 0];
    let mut o = Oracle {
        report_used: true,
        ..Default::default()
    };
    o.alts.insert(
        k.clone(),
        vec![alt(&[[0, 0, 0], [1, 0, 0], [0, 1, -1]], 40)],
    );
    let p = SeedMinPolicy::default();
    let emu = minimise_key(
        &k,
        &centre,
        &shell(),
        &p,
        &mut o.clone(),
        &mut StubSystemPoles,
    )
    .unwrap();
    let mut q = p.clone();
    q.backend = "engine".into();
    // the emulating oracle has no minimiser: typed refusal
    assert!(matches!(
        minimise_key(
            &k,
            &centre,
            &shell(),
            &q,
            &mut o.clone(),
            &mut StubSystemPoles
        ),
        Err(SeedMinError::EngineLacksMinimiser { .. })
    ));
    let mut fe = FakeEngine { oracle: o.clone() };
    let eng = minimise_key(&k, &centre, &shell(), &q, &mut fe, &mut StubSystemPoles).unwrap();
    assert_eq!(eng.backend, "engine");
    assert_eq!(eng.trials_run, 1);
    assert_eq!(eng.result_record(), emu.result_record());
}

#[test]
fn rows_at_d0_is_refused_typed_without_probe_mode_and_is_never_a_verdict() {
    let k: Key = vec![2, 2, 0];
    let mut o = Oracle::default();
    // normal mode: the smallest relation (centre only) is SINGULAR; a regular one needs two more seeds
    let mut sing = alt(&[[0, 0, 0]], 3);
    sing.pole = PoleOrder::Order { order: 2 };
    sing.probe = "NO_RELATION_AT_D0";
    o.alts.insert(
        k.clone(),
        vec![alt(&[[0, 0, 0], [1, 0, 0], [0, 1, 0]], 9), sing.clone()],
    );
    // the normal pass with pole_objective off lands on the singular relation
    let p = SeedMinPolicy {
        rows_at_d0: true,
        pole_objective: "off".into(),
        ..SeedMinPolicy::default()
    };
    let e = minimise_key(&k, &C0, &shell(), &p, &mut o.clone(), &mut StubSystemPoles).unwrap_err();
    assert!(matches!(e, SeedMinError::EngineLacksProbeMode { .. }));
    assert!(e.to_string().contains("engine lacks probe mode"));
    // with probe mode: the d0 pass yields a candidate; it is accepted only after the exact solve, by the objective
    let mut op = o.clone();
    op.caps = EngineCaps {
        probe_d: true,
        features: vec!["probe_d".into()],
        ..Default::default()
    };
    let mut q = p.clone();
    q.pole_objective = "on".into();
    q.monotone_rhs = false;
    let mut normal = q.clone();
    normal.rows_at_d0 = false;
    let n = minimise_key(
        &k,
        &C0,
        &shell(),
        &normal,
        &mut op.clone(),
        &mut StubSystemPoles,
    )
    .unwrap();
    let d = minimise_key(&k, &C0, &shell(), &q, &mut op.clone(), &mut StubSystemPoles).unwrap();
    let rec = d.rows_at_d0.as_ref().unwrap();
    assert!(
        rec["verdict"]
            .as_str()
            .unwrap()
            .starts_with("none in either direction")
    );
    assert_eq!(
        n.list, d.list,
        "here the normal pass already keeps the regular rows"
    );
    // a probe that never answers EXISTS_AT_D0: no candidate, NO verdict, the normal result unchanged
    let mut onone = op.clone();
    for a in onone.alts.get_mut(&k).unwrap() {
        a.probe = "NO_RELATION_AT_D0";
    }
    let d2 = minimise_key(&k, &C0, &shell(), &q, &mut onone, &mut StubSystemPoles).unwrap();
    assert_eq!(d2.rows_at_d0.as_ref().unwrap()["outcome"], "no_candidate");
    assert_eq!(d2.result_record(), n.result_record());
    // a probe candidate whose exact solve is SINGULAR (rows at d0 prove nothing): not adopted against a regular one
    let mut olie = op.clone();
    {
        let v = olie.alts.get_mut(&k).unwrap();
        v[1].probe = "EXISTS_AT_D0"; // the singular alternative "exists at d0"
        v[0].probe = "EXISTS_AT_D0";
    }
    let d3 = minimise_key(&k, &C0, &shell(), &q, &mut olie, &mut StubSystemPoles).unwrap();
    let r3 = d3.rows_at_d0.as_ref().unwrap();
    assert_eq!(r3["outcome"], "candidate_not_better");
    assert_eq!(d3.relation.as_ref().unwrap().pole, PoleOrder::regular());
}

#[test]
fn policy_and_input_errors_are_typed() {
    let k: Key = vec![2, 2, 0];
    let mut o = Oracle::default();
    o.alts.insert(k.clone(), vec![alt(&[[0, 0, 0]], 1)]);
    let p = SeedMinPolicy::default();
    assert!(matches!(
        minimise_key(&k, &C0, &[], &p, &mut o, &mut StubSystemPoles),
        Err(SeedMinError::Input { .. })
    ));
    assert!(matches!(
        minimise_key(&k, &[0, 0], &shell(), &p, &mut o, &mut StubSystemPoles),
        Err(SeedMinError::Input { .. })
    ));
    let dup = vec![vec![0, 0, 0], vec![0, 0, 0]];
    assert!(matches!(
        minimise_key(&k, &C0, &dup, &p, &mut o, &mut StubSystemPoles),
        Err(SeedMinError::Input { .. })
    ));
    // a key the start list does not solve: no_start, no minimise member
    let km = minimise_key(&[9, 9, 9], &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(km.stopped, "no_start");
    assert!(km.minimise.is_none());
    // the budget: max_solves 2 stops the greedy pass
    let mut b = p.clone();
    b.max_solves = 2;
    let km = minimise_key(&k, &C0, &shell(), &b, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(km.stopped, "max_solves");
    assert_eq!(km.solves, 2);
    assert_eq!(km.minimise.as_ref().unwrap()["stopped"], "max_solves");
}

#[test]
fn row_order_is_a_parameter_and_is_kept() {
    let k: Key = vec![2, 2, 0];
    let mut o = Oracle::default();
    o.alts
        .insert(k.clone(), vec![alt(&[[0, 0, 0], [1, 1, 1], [1, 0, 0]], 4)]);
    let mut p = SeedMinPolicy {
        row_order: "distance_desc".into(),
        ..SeedMinPolicy::default()
    };
    let km = minimise_key(&k, &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(km.list, vec![vec![1, 1, 1], vec![1, 0, 0], vec![0, 0, 0]]);
    assert_eq!(km.result_record()["row_order"], "distance_desc");
    p.row_order = "distance_asc".into();
    let km = minimise_key(&k, &C0, &shell(), &p, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(km.list, vec![vec![0, 0, 0], vec![1, 0, 0], vec![1, 1, 1]]);
}
