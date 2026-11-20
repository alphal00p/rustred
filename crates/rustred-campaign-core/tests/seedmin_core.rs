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
    // the POLE-FIRST objective with keep_seed: the default until 2026-11-19, EXPLICIT since rule 21 (the default puts
    // terms first and lets an undecided regularity drop)
    let p = SeedMinPolicy {
        objective: vec!["pole_order".into(), "seeds".into(), "terms".into()],
        on_undecided_regularity: "keep_seed".into(),
        ..SeedMinPolicy::default()
    };
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
    let p = SeedMinPolicy {
        system_regularity: "on".into(),
        ..SeedMinPolicy::default()
    };
    let km = minimise_key(&k, &C0, &shell(), &p, &mut o, &mut NeedsSeed(vec![1, 1, 1])).unwrap();
    // off (the default): the system judge is not consulted
    let off = SeedMinPolicy::default();
    let km_off = minimise_key(
        &k,
        &C0,
        &shell(),
        &off,
        &mut o,
        &mut NeedsSeed(vec![1, 1, 1]),
    )
    .unwrap();
    assert_eq!(km_off.list, vec![vec![0, 0, 0]]);
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
    // the POLE-FIRST objective (explicit since rule 21) decides the d0 candidate below
    let p = SeedMinPolicy {
        rows_at_d0: true,
        pole_objective: "off".into(),
        objective: vec!["pole_order".into(), "seeds".into(), "terms".into()],
        on_undecided_regularity: "keep_seed".into(),
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

#[test]
fn halving_chunks_reach_the_same_lists_with_fewer_solves() {
    let k: Key = vec![3, 2, -1];
    let mut o = Oracle::default();
    o.alts.insert(
        k.clone(),
        vec![alt(&[[0, 0, 0], [1, 0, 0], [0, 1, -1]], 40)],
    );
    let single = SeedMinPolicy::default();
    let halving = SeedMinPolicy {
        drop_chunks: "halving".into(),
        ..SeedMinPolicy::default()
    };
    let a = minimise_key(&k, &C0, &shell(), &single, &mut o, &mut StubSystemPoles).unwrap();
    let b = minimise_key(&k, &C0, &shell(), &halving, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(a.list, b.list);
    assert!(
        b.solves < a.solves,
        "halving {} vs single {}",
        b.solves,
        a.solves
    );
    assert_eq!(b.minimise.as_ref().unwrap()["drop_chunks"], "halving");
    assert!(a.minimise.as_ref().unwrap().get("drop_chunks").is_none());
    // joint: the same pieces
    let ks: Vec<Key> = vec![
        vec![2, 1, 0],
        vec![3, 1, 0],
        vec![4, 1, 0],
        vec![5, 1, 0],
        vec![6, 1, 0],
    ];
    let mut oj = Oracle::default();
    let private = [[1, 0, 0], [0, 1, 0], [0, 0, -1], [1, 1, 0], [1, 0, -1]];
    for (k, pr) in ks.iter().zip(private) {
        oj.alts.insert(k.clone(), vec![alt(&[[0, 0, 0], pr], 7)]);
    }
    let ja = minimise_joint(
        &ks,
        &C0,
        &shell(),
        &single,
        &mut oj.clone(),
        &mut StubSystemPoles,
    )
    .unwrap();
    let halving = SeedMinPolicy {
        drop_chunks: "halving".into(),
        joint: rustred_campaign_core::seedmin::policy::JointPolicy {
            drop_chunks: "halving".into(),
            ..Default::default()
        },
        ..SeedMinPolicy::default()
    };
    let jb = minimise_joint(
        &ks,
        &C0,
        &shell(),
        &halving,
        &mut oj.clone(),
        &mut StubSystemPoles,
    )
    .unwrap();
    assert_eq!(ja.pieces[0].list, jb.pieces[0].list);
    assert!(
        jb.solves < ja.solves,
        "halving {} vs single {}",
        jb.solves,
        ja.solves
    );
}

/// A mock p* oracle: Order(0) while the system holds a seed equal to `regular_seed`, Order(1) otherwise.
struct MockTadic {
    regular_seed: Vec<i64>,
}

impl rustred_campaign_core::tadic::PoleOrderOracle for MockTadic {
    fn best_pole_order(
        &self,
        sys: &rustred_campaign_core::tadic::ExactSystem,
        q: &rustred_campaign_core::tadic::PoleQuery<'_>,
    ) -> rustred_campaign_core::tadic::PoleOrderResult {
        use rustred_campaign_core::tadic::*;
        let p = if sys.seeds.contains(&self.regular_seed) {
            0
        } else {
            1
        };
        PoleOrderResult {
            schema: TADIC_VERSION,
            system: sys.id.clone(),
            target: q.target.to_vec(),
            d0: q.d0.text(),
            arith: Arith::Exact,
            verdict: Verdict::Exact,
            p_star: Some(PStar::Order(p)),
            relation: None,
            rank: Default::default(),
            screen_at_d0: None,
            // int7: tadic v1.1 field (no relation asked: none)
            relation_seeds: None,
            seconds: 0.0,
            notes: vec![],
        }
    }
}

#[test]
fn the_tadic_system_judge_bounds_drops_by_the_start_systems_p_star() {
    use rustred_campaign_core::replay_base::family::Family;
    use rustred_campaign_core::seedmin::TadicSystem;
    const SUNSET: &str = "I(name(S),loops(k1,k2),externals(q),dimension(d),prop(D1,(k1)^2,1),prop(D2,(k2)^2,1),prop(D3,(k1+k2+q)^2,1),prop(D4,(k1+q)^2,0),prop(D5,(k2+q)^2,0),gram(q,q,1))";
    let fam = Family::parse(SUNSET).unwrap();
    let lines = vec![true, true, true, false, false];
    let key = vec![2, 1, 1, -1, 0];
    let lower = |_k: &[i64], _t: &[i64]| true;
    // the system holding the seed key + [1,0,0,0,0] is "regular"; dropping that seed raises p* to 1: refused
    let oracle = MockTadic {
        regular_seed: vec![3, 1, 1, -1, 0],
    };
    let mut ts = TadicSystem::new(&oracle, &fam, lines, &lower, Default::default(), true);
    let before = vec![
        vec![0, 0, 0, 0, 0],
        vec![1, 0, 0, 0, 0],
        vec![0, 1, 0, 0, 0],
        vec![0, 0, -1, 0, 0],
    ];
    let drop_regular = vec![
        vec![0, 0, 0, 0, 0],
        vec![0, 1, 0, 0, 0],
        vec![0, 0, -1, 0, 0],
    ];
    let drop_other = vec![
        vec![0, 0, 0, 0, 0],
        vec![1, 0, 0, 0, 0],
        vec![0, 0, -1, 0, 0],
    ];
    assert_eq!(
        ts.keeps_regularity(&before, &drop_regular, &key, "4"),
        Some(false)
    );
    assert_eq!(
        ts.keeps_regularity(&before, &drop_other, &key, "4"),
        Some(true)
    );
    // a seed outside the top sector at the key (line 1 at 0) is filtered as the engine's sector filter does
    let outside = vec![
        vec![0, 0, 0, 0, 0],
        vec![1, 0, 0, 0, 0],
        vec![0, -1, 0, 0, 0],
    ];
    assert_eq!(
        ts.keeps_regularity(&before, &outside, &key, "4"),
        Some(true)
    );
    assert!(ts.seconds.len() >= 3);
}

/// A coreseed CoreSolver oracle for the BRIDGE (one key): the key solves iff the list (offsets from the key) contains
/// `needs`; with `engine` a request carrying `elimination.minimise` is answered like the engine verb: the minimisation
/// run inside (the emulation through the bridge on a plain copy of this oracle) and the contract's `minimise` member
/// (+ the proposed `seeds`, offsets from the key) in the record.
#[derive(Clone)]
struct CoreOracle {
    needs: Vec<Offset>,
    engine: bool,
}

fn core_info(
    needs: &[Offset],
    k: &[i32],
    rhs: u64,
    minimise: Option<Value>,
) -> rustred_campaign_core::coreseed::RelationInfo {
    rustred_campaign_core::coreseed::RelationInfo {
        relation_digest: sha256_hex(format!("{k:?}/{needs:?}").as_bytes()),
        rhs: vec![],
        rhs_count: rhs,
        seeds_total: None,
        seeds_used: Some(needs.to_vec()),
        d0: rustred_campaign_core::coreseed::D0Verdict::Regular,
        recipe_digest: "r".into(),
        determinism_id: "fake".into(),
        relation_rule_taken: None,
        minimise,
        record_sha256: "s".into(),
    }
}

fn core_make(
    list: &[Offset],
    keys: &[Key],
    label: &str,
    member: Option<Value>,
) -> Result<rustred_campaign_core::coreseed::SolveReq, String> {
    let mut recipe = json!({"seeds": {"centres": list}, "elimination": {"convention": "rustred.recipe-elim.v1"}});
    if let Some(m) = member {
        recipe["elimination"]["minimise"] = m;
    }
    Ok(rustred_campaign_core::coreseed::SolveReq {
        label: label.into(),
        text: recipe.to_string(),
        recipe,
        keys: keys.to_vec(),
        relation_rule: "first_admissible".into(),
    })
}

impl rustred_campaign_core::coreseed::CoreSolver for CoreOracle {
    fn solve(
        &mut self,
        batch: &[rustred_campaign_core::coreseed::SolveReq],
    ) -> Result<Vec<Vec<rustred_campaign_core::coreseed::KeyAnswer>>, String> {
        use rustred_campaign_core::coreseed::KeyAnswer;
        let mut out = vec![];
        for r in batch {
            let list: Vec<Offset> =
                serde_json::from_value(r.recipe["seeds"]["centres"].clone()).unwrap();
            let member = r.recipe["elimination"].get("minimise").cloned();
            let mut v = vec![];
            for k in &r.keys {
                if self.engine && member.is_some() {
                    let mut inner = CoreOracle {
                        engine: false,
                        ..self.clone()
                    };
                    let pol = rustred_campaign_core::coreseed::spec::CoreSeedPolicy::default();
                    let res = rustred_campaign_core::seedmin::bridge::minimise(
                        &mut inner,
                        &pol,
                        &core_make,
                        &list,
                        std::slice::from_ref(k),
                        None,
                        "engine-inside",
                    )?
                    .expect("solves");
                    let mut m = res.member.clone();
                    m["seeds"] = json!(res.list);
                    let info = res.per_key[k].clone();
                    v.push(KeyAnswer::Relation(Box::new(core_info(
                        &self.needs,
                        k,
                        info.rhs_count,
                        Some(m),
                    ))));
                } else if self.needs.iter().all(|o| list.contains(o)) {
                    v.push(KeyAnswer::Relation(Box::new(core_info(
                        &self.needs,
                        k,
                        40,
                        None,
                    ))));
                } else {
                    v.push(KeyAnswer::NoRelation);
                }
            }
            out.push(v);
        }
        Ok(out)
    }
}

#[test]
fn through_the_bridge_the_engine_verb_and_the_emulation_give_the_same_result() {
    let k: Key = vec![3, 2, -1];
    let needs: Vec<Offset> = vec![vec![0, 0, 0], vec![1, 0, 0], vec![0, 1, -1]];
    let list = shell();
    let emu_pol = rustred_campaign_core::coreseed::spec::CoreSeedPolicy::default();
    let mut eng_pol = emu_pol.clone();
    eng_pol.minimise.backend = "engine".into();
    let emu = rustred_campaign_core::seedmin::bridge::minimise(
        &mut CoreOracle {
            needs: needs.clone(),
            engine: false,
        },
        &emu_pol,
        &core_make,
        &list,
        std::slice::from_ref(&k),
        None,
        "emu",
    )
    .unwrap()
    .expect("the emulation minimises");
    let eng = rustred_campaign_core::seedmin::bridge::minimise(
        &mut CoreOracle {
            needs: needs.clone(),
            engine: true,
        },
        &eng_pol,
        &core_make,
        &list,
        std::slice::from_ref(&k),
        None,
        "eng",
    )
    .unwrap()
    .expect("the engine backend returns the minimised relation (not None, not the start list)");
    assert_eq!(eng.list, emu.list);
    assert_eq!(eng.list.len(), 3);
    assert_eq!(
        eng.per_key[&k].relation_digest,
        emu.per_key[&k].relation_digest
    );
    let strip = |m: &Value| {
        let mut m = m.clone();
        let o = m.as_object_mut().unwrap();
        o.remove("seeds");
        o.remove("backend");
        m
    };
    assert_eq!(strip(&eng.member), strip(&emu.member));
    assert_eq!(eng.member["backend"], "engine");
    assert_eq!(emu.member["backend"], "emulated");
}

#[test]
fn augmentation_never_minimises_the_same_key_twice() {
    // A's minimised list {c, a} solves A; joined with B's list the first match at A becomes a SINGULAR alternative
    // (needs c + b): A then fails the union's confirmation; it must not be minimised again
    let ka: Key = vec![2, 1, 0];
    let kb: Key = vec![3, 1, 0];
    let mut o = Oracle {
        report_used: true,
        ..Default::default()
    };
    let mut sing = alt(&[[0, 0, 0], [0, 1, -1]], 5);
    sing.pole = PoleOrder::Order { order: 1 };
    o.alts
        .insert(ka.clone(), vec![sing, alt(&[[0, 0, 0], [1, 0, 0]], 8)]);
    o.alts
        .insert(kb.clone(), vec![alt(&[[0, 0, 0], [0, 1, -1]], 9)]);
    let mut p = SeedMinPolicy::default();
    p.joint.spread.count = 1;
    let start: Vec<Offset> = vec![vec![0, 0, 0], vec![1, 0, 0]];
    // the start list solves A (regular) but not B; give B its own seed so the start serves both
    let mut list = start.clone();
    list.push(vec![0, 1, -1]);
    let jm = minimise_joint(
        &[ka.clone(), kb.clone()],
        &C0,
        &list,
        &p,
        &mut o,
        &mut StubSystemPoles,
    )
    .unwrap();
    let spread_keys: Vec<Value> = jm.record["spread"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["key"].clone())
        .collect();
    let uniq: BTreeSet<String> = spread_keys.iter().map(|k| k.to_string()).collect();
    assert_eq!(
        uniq.len(),
        spread_keys.len(),
        "a key minimised twice: {spread_keys:?}"
    );
    // every key is still served (a piece or the start list), never lost
    let covered: usize = jm.pieces.iter().map(|p| p.keys.len()).sum();
    assert_eq!(covered, 2);
}

#[test]
fn shell_subsets_are_alternatives_ranked_by_coverage_poles_and_size() {
    use rustred_campaign_core::seedmin::{ShellSubsetSpec, minimise_shell_subsets};
    let lines = vec![true, true, false];
    let k: Key = vec![2, 2, -1];
    let mut o = Oracle::default();
    // with an S2 seed present the first match is SINGULAR; with only the Sm1 seed the relation is REGULAR
    let mut sing = alt(&[[0, 0, 0], [2, 0, 0]], 5);
    sing.pole = PoleOrder::Order { order: 1 };
    o.alts
        .insert(k.clone(), vec![sing, alt(&[[0, 0, 0], [-1, 0, 0]], 10)]);
    let spec = ShellSubsetSpec {
        base: vec!["S0".into(), "S1".into()],
        escalation: vec!["S2".into(), "Sm1".into()],
        order: "smallest_first".into(),
        rank: vec![
            "unserved".into(),
            "pieces".into(),
            "max_pole".into(),
            "singular_keys".into(),
            "seeds".into(),
            "max_terms".into(),
        ],
    };
    assert_eq!(
        spec.subsets(),
        vec![
            vec!["S2".to_string()],
            vec!["Sm1".to_string()],
            vec!["S2".to_string(), "Sm1".to_string()]
        ]
    );
    let alts = minimise_shell_subsets(
        std::slice::from_ref(&k),
        &C0,
        &lines,
        &spec,
        &SeedMinPolicy::default(),
        &mut o,
        &mut StubSystemPoles,
    )
    .unwrap();
    assert_eq!(alts.len(), 3);
    // best: base + Sm1 (regular); the two lists containing S2 deliver the singular relation
    assert_eq!(alts[0].shells, vec!["S0", "S1", "Sm1"]);
    assert_eq!(alts[0].measures["max_pole"], 0);
    assert_eq!(
        alts[0].joint.pieces[0].list,
        vec![vec![0, 0, 0], vec![-1, 0, 0]]
    );
    assert!(alts[1..].iter().all(|a| a.measures["max_pole"] == 1));
    let mut bad = spec.clone();
    bad.rank = vec!["beauty".into()];
    assert!(
        minimise_shell_subsets(
            std::slice::from_ref(&k),
            &C0,
            &lines,
            &bad,
            &SeedMinPolicy::default(),
            &mut o,
            &mut StubSystemPoles
        )
        .is_err()
    );
}

#[test]
fn undecided_answers_are_never_journaled_nor_replayed() {
    use rustred_campaign_core::seedmin::journal::JournalLine;
    let k: Key = vec![2, 2, 0];
    let mut o = Oracle::default();
    o.alts.insert(k.clone(), vec![alt(&[[0, 0, 0]], 1)]);
    // a journal holding an UNDECIDED start answer (an interrupted job) must not be replayed
    let t = Trial {
        label: "x".into(),
        centre: C0.to_vec(),
        list: shell(),
        keys: vec![k.clone()],
        mode: TrialMode::Solve,
        engine_minimise: None,
    };
    let line = serde_json::to_string(&JournalLine {
        digest: t.digest(),
        label: "x".into(),
        answers: vec![TrialAnswer::Undecided {
            class: "refused".into(),
            message: "killed".into(),
        }],
    })
    .unwrap();
    let mut js = JournaledSolver::from_lines(o.clone(), [line.as_str()]).unwrap();
    assert!(js.is_empty());
    let a = js.solve(std::slice::from_ref(&t)).unwrap();
    assert!(a[0][0].relation().is_some());
    // an undecided answer from the inner solver is returned but not journaled
    let mut o2 = o.clone();
    o2.fail_after = None;
    o2.alts.clear();
    struct Und;
    impl TrialSolver for Und {
        fn solve(&mut self, t: &[Trial]) -> Result<Vec<Vec<TrialAnswer>>, String> {
            Ok(t.iter()
                .map(|x| {
                    x.keys
                        .iter()
                        .map(|_| TrialAnswer::Undecided {
                            class: "budget_wall".into(),
                            message: String::new(),
                        })
                        .collect()
                })
                .collect())
        }
        fn capabilities(&self) -> EngineCaps {
            EngineCaps::default()
        }
    }
    let mut ju = JournaledSolver::new(Und);
    let a = ju.solve(std::slice::from_ref(&t)).unwrap();
    assert!(matches!(a[0][0], TrialAnswer::Undecided { .. }));
    assert!(ju.take_new_lines().is_empty());
    assert!(ju.is_empty());
}

#[test]
fn the_core_solver_entry_for_shell_subsets_ranks_the_alternatives() {
    let k: Key = vec![2, 2, -1];
    let mut o = CoreOracle {
        needs: vec![vec![0, 0, 0], vec![-1, 0, 0]],
        engine: false,
    };
    let pol = rustred_campaign_core::coreseed::spec::CoreSeedPolicy::default();
    let (alts, infos) = rustred_campaign_core::seedmin::bridge::minimise_shell_subsets_core(
        &mut o,
        &pol,
        &core_make,
        std::slice::from_ref(&k),
        &C0,
        &[true, true, false],
        &["S0".to_string(), "S1".to_string()],
        &["S2".to_string(), "Sm1".to_string()],
    )
    .unwrap();
    assert_eq!(alts.len(), 3);
    assert_eq!(alts[0].shells, vec!["S0", "S1", "Sm1"]);
    assert_eq!(
        alts[0].joint.pieces[0].list,
        vec![vec![0, 0, 0], vec![-1, 0, 0]]
    );
    // the S2-only list solves nothing: ranked last (unserved 1)
    assert_eq!(alts[2].shells, vec!["S0", "S1", "S2"]);
    assert_eq!(alts[2].measures["unserved"], 1);
    assert!(!infos.is_empty());
}

#[test]
fn the_joint_strategy_is_data_auto_switches_to_halving_above_the_threshold() {
    use rustred_campaign_core::seedmin::policy::JointPolicy;
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
    let with = |mode: &str, above: usize| SeedMinPolicy {
        joint: JointPolicy {
            drop_chunks: mode.into(),
            halving_above_keys: above,
            ..Default::default()
        },
        ..SeedMinPolicy::default()
    };
    let single = minimise_joint(
        &ks,
        &C0,
        &shell(),
        &with("single", 0),
        &mut o.clone(),
        &mut StubSystemPoles,
    )
    .unwrap();
    let auto_small = minimise_joint(
        &ks,
        &C0,
        &shell(),
        &with("auto", 50),
        &mut o.clone(),
        &mut StubSystemPoles,
    )
    .unwrap();
    let auto_large = minimise_joint(
        &ks,
        &C0,
        &shell(),
        &with("auto", 4),
        &mut o.clone(),
        &mut StubSystemPoles,
    )
    .unwrap();
    let halving = minimise_joint(
        &ks,
        &C0,
        &shell(),
        &with("halving", 0),
        &mut o.clone(),
        &mut StubSystemPoles,
    )
    .unwrap();
    // same lists; auto below the threshold = single, above = halving
    assert_eq!(single.pieces[0].list, halving.pieces[0].list);
    assert_eq!(
        auto_small.record["pieces"][0]["minimise"],
        single.record["pieces"][0]["minimise"]
    );
    assert_eq!(
        auto_large.record["pieces"][0]["minimise"]["solves"],
        halving.record["pieces"][0]["minimise"]["solves"]
    );
    assert!(SeedMinPolicy::parse(r#"{"joint":{"drop_chunks":"sometimes"}}"#).is_err());
    assert_eq!(SeedMinPolicy::default().joint.drop_chunks, "auto");
}

/// An oracle whose answer at ONE key fails in the FIRST final-verification trial only (an engine that does not
/// reproduce at the last moment): the joint minimiser must not claim the list there.
struct FlakyAtVerify {
    inner: Oracle,
    key: Key,
    failed: bool,
}

impl TrialSolver for FlakyAtVerify {
    fn solve(&mut self, trials: &[Trial]) -> Result<Vec<Vec<TrialAnswer>>, String> {
        let mut out = self.inner.solve(trials)?;
        for (t, a) in trials.iter().zip(out.iter_mut()) {
            if !self.failed && t.label.ends_with("/verify") {
                for (k, x) in t.keys.iter().zip(a.iter_mut()) {
                    if *k == self.key {
                        *x = TrialAnswer::NoRelation;
                        self.failed = true;
                    }
                }
            }
        }
        Ok(out)
    }
    fn capabilities(&self) -> EngineCaps {
        self.inner.capabilities()
    }
}

/// planfix (audit r2-planners S2): the joint minimiser's FINAL VERIFICATION solves EVERY key of the piece and hands on
/// a key where it fails, even when every earlier trial at that key succeeded. The key that fails is the LAST of the
/// piece, so a verification of a prefix of the keys (the auditor's surviving mutation M-seedmin-2: the first key only)
/// would claim the list there: the piece would hold the key, its per-key facts would not cover the piece's keys, and the
/// record's verify trial would not list every key. Each assertion below catches it.
#[test]
fn the_final_verification_solves_every_key_and_hands_on_a_key_that_fails_there() {
    let mut o = Oracle::default();
    let privs: [[i32; 3]; 4] = [[1, 0, 0], [0, 1, 0], [0, 0, -1], [1, 1, 0]];
    let mut keys = vec![];
    for (i, p) in privs.iter().enumerate() {
        let k: Key = vec![2 + i as i32, 1, 0];
        o.alts.insert(k.clone(), vec![alt(&[[0, 0, 0], *p], 10)]);
        keys.push(k);
    }
    let last = keys.last().unwrap().clone();
    let mut fl = FlakyAtVerify {
        inner: o,
        key: last.clone(),
        failed: false,
    };
    let p = SeedMinPolicy::default();
    let jm = minimise_joint(&keys, &C0, &shell(), &p, &mut fl, &mut StubSystemPoles).unwrap();
    assert!(fl.failed, "the verify trial reached the last key");
    let first = &jm.pieces[0];
    assert!(
        !first.keys.contains(&last),
        "claimed at a key whose verification failed: {:?}",
        first.keys
    );
    assert_eq!(first.keys.len(), keys.len() - 1);
    assert_eq!(
        first.per_key.len(),
        first.keys.len(),
        "one verified relation per piece key"
    );
    assert_eq!(
        first.domain_tested["verification_failed"],
        json!([last.clone()])
    );
    // the key is handed on (served by a later piece, verified there), never lost
    assert!(
        jm.pieces[1..].iter().any(|pc| pc.keys.contains(&last)) || jm.unserved.contains(&last),
        "{:?}",
        jm.pieces.iter().map(|p| p.keys.clone()).collect::<Vec<_>>()
    );
    // the record's first verify trial covers every key of the domain
    let verify: Vec<&Value> = jm.record["trials"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["kind"] == "verify")
        .collect();
    assert!(!verify.is_empty());
    assert_eq!(verify[0]["keys"], keys.len());
}

/// planfix (audit r2-planners S1): a piece's list beyond its own keys is compared key by key with the FULL list; a key
/// where the minimised list is singular and the full list regular is LISTED (the objective never judged it), and the
/// claim says the objective applied at the piece's keys only.
#[test]
fn beyond_its_piece_a_list_is_compared_with_the_full_list_and_worse_keys_are_listed() {
    use rustred_campaign_core::seedmin::{Piece, verify_beyond_piece};
    let mut o = Oracle::default();
    let own: Key = vec![2, 1, 0];
    let same: Key = vec![3, 1, 0];
    let worse: Key = vec![4, 1, 0];
    let lost: Key = vec![5, 1, 0];
    o.alts.insert(own.clone(), vec![alt(&[[0, 0, 0]], 5)]);
    o.alts.insert(same.clone(), vec![alt(&[[0, 0, 0]], 6)]);
    // at `worse`: the regular relation needs [1,0,0] (only in the full list); the minimised list gets a singular one
    let mut sing = alt(&[[0, 0, 0]], 4);
    sing.pole = PoleOrder::Order { order: 1 };
    o.alts
        .insert(worse.clone(), vec![alt(&[[0, 0, 0], [1, 0, 0]], 9), sing]);
    o.alts.insert(lost.clone(), vec![alt(&[[0, 1, 0]], 9)]);
    let piece = Piece {
        label: "0".into(),
        keys: vec![own.clone()],
        list: vec![vec![0, 0, 0]],
        per_key: vec![],
        how: "joint".into(),
        minimise: None,
        domain_tested: json!({}),
        rows_at_d0: None,
    };
    let extra = vec![own.clone(), same.clone(), worse.clone(), lost.clone()];
    let r = verify_beyond_piece(&piece, &C0, Some(&shell()), &extra, 2, &mut o).unwrap();
    assert_eq!(
        (r.own_keys, r.extra_keys),
        (1, 3),
        "the piece's own key is not re-counted"
    );
    assert_eq!((r.extra_valid, r.extra_regular), (2, 1));
    assert_eq!((r.full_valid, r.full_regular), (Some(3), Some(3)));
    assert_eq!(
        r.worse_than_full,
        vec![(
            worse.clone(),
            PoleOrder::Order { order: 1 },
            PoleOrder::regular()
        )]
    );
    assert_eq!(r.failed, vec![(lost.clone(), "no_relation".to_string())]);
    assert!(
        r.claim
            .starts_with("pole objective applied at the 1 piece keys only"),
        "{}",
        r.claim
    );
    assert!(
        r.claim.contains("WORSE than the full list at 1 key(s)"),
        "{}",
        r.claim
    );
    // without the comparison: counts only, and the claim says so
    let r2 = verify_beyond_piece(&piece, &C0, None, &extra, 40, &mut o).unwrap();
    assert!(r2.worse_than_full.is_empty() && r2.full_valid.is_none());
    assert!(
        r2.claim.ends_with("not compared with the full list"),
        "{}",
        r2.claim
    );
}

/// planfix (audit r2-planners ITEM 6): the halving factor is a policy parameter (default 2, omitted from the canonical
/// policy at its default so existing digests / journals stay valid; below 2 refused); another factor reaches the same
/// list (the factor changes the cost, never the result of the drop test).
#[test]
fn the_halving_factor_is_a_parameter_with_a_digest_stable_default() {
    let d = SeedMinPolicy::default();
    assert_eq!(d.halving_factor, 2);
    let explicit = SeedMinPolicy::parse(r#"{"halving_factor": 2}"#).unwrap();
    assert_eq!(
        explicit.digest(),
        d.digest(),
        "the default is not part of the digest"
    );
    assert!(
        !serde_json::to_string(&d)
            .unwrap()
            .contains("halving_factor")
    );
    let e = SeedMinPolicy::parse(r#"{"halving_factor": 1}"#).unwrap_err();
    assert!(e.contains("halving_factor"), "{e}");
    let three = SeedMinPolicy::parse(r#"{"halving_factor": 3, "drop_chunks": "halving"}"#).unwrap();
    assert_ne!(three.digest(), d.digest());
    let k: Key = vec![3, 2, -1];
    let mut o = Oracle::default();
    o.alts.insert(
        k.clone(),
        vec![alt(&[[0, 0, 0], [1, 0, 0], [0, 1, -1]], 40)],
    );
    let two = SeedMinPolicy {
        drop_chunks: "halving".into(),
        ..SeedMinPolicy::default()
    };
    let a = minimise_key(&k, &C0, &shell(), &two, &mut o, &mut StubSystemPoles).unwrap();
    let b = minimise_key(&k, &C0, &shell(), &three, &mut o, &mut StubSystemPoles).unwrap();
    assert_eq!(a.list, b.list);
}
