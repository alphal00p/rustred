//! Lane coreseed (interfaces/coreseed.md): the IBP-core flow on a TOY chart (arity 4: two lines, two numerators; nothing
//! five-loop specific) with an in-memory oracle solver: a key's relation exists iff the solved list contains the seeds
//! the oracle names (first option in order); the support (`seeds_used`) is exactly those seeds.

use std::collections::{BTreeMap, BTreeSet};

use rustred_campaign_core::coreseed::spec::CoreSeedPolicy;
use rustred_campaign_core::coreseed::{
    CoreReport, CoreSolver, D0Verdict, Key, KeyAnswer, Offset, RelationInfo, SolveReq, solve_core,
};
use serde_json::Value;

const LINES: [bool; 4] = [true, true, false, false];

#[derive(Clone)]
struct Opt {
    needs: Vec<Offset>,
    rhs: Vec<Key>,
    d0: D0Verdict,
}

#[derive(Clone, Default)]
struct Oracle {
    opts: BTreeMap<Key, Vec<Opt>>,
    /// keys answered BUDGET_WALL at every solve
    budget: BTreeSet<Key>,
    /// keys answered BUDGET_WALL when the list has more seeds than this
    budget_above: BTreeMap<Key, usize>,
}

struct FakeSolver {
    oracle: Oracle,
    batches: Vec<Vec<String>>,
    /// stop with an error after this many batches (an interrupted run)
    fail_after: Option<usize>,
    journal: BTreeMap<String, Vec<KeyAnswer>>,
    fresh: usize,
}

fn list_of(req: &SolveReq) -> Vec<Offset> {
    serde_json::from_value(req.recipe["seeds"]["centres"].clone()).unwrap()
}

fn answer(o: &Oracle, req: &SolveReq, k: &Key) -> KeyAnswer {
    let list = list_of(req);
    let valid: BTreeSet<Offset> = list
        .iter()
        .filter(|off| {
            k.iter()
                .zip(off.iter())
                .zip(LINES)
                .all(|((&a, &b), l)| if l { a + b >= 1 } else { a + b <= 0 })
        })
        .cloned()
        .collect();
    if o.budget.contains(k) || o.budget_above.get(k).is_some_and(|&n| valid.len() > n) {
        return KeyAnswer::Budget {
            what: "BUDGET_WALL".into(),
        };
    }
    let engine_min = !req.recipe["elimination"]["minimise"].is_null();
    for (i, op) in o.opts.get(k).into_iter().flatten().enumerate() {
        if op.needs.iter().all(|n| valid.contains(n)) {
            return KeyAnswer::Relation(Box::new(RelationInfo {
                relation_digest: format!("{k:?}#{i}"),
                rhs: op.rhs.clone(),
                rhs_count: op.rhs.len() as u64,
                seeds_total: Some(valid.len() as u64),
                seeds_used: Some(op.needs.clone()),
                d0: op.d0.clone(),
                recipe_digest: "r".into(),
                determinism_id: "fake".into(),
                relation_rule_taken: Some("first_admissible".into()),
                // the engine minimiser's answer (line-7 member): the support IS the minimised list
                minimise: engine_min.then(|| {
                    serde_json::json!({"mode": "greedy", "measure": "comparator", "order": "distance",
                        "solves": 1, "stopped": "done", "seeds_before": valid.len(), "seeds_after": op.needs.len()})
                }),
                record_sha256: format!("{k:?}#{i}"),
            }));
        }
    }
    KeyAnswer::NoRelation
}

impl CoreSolver for FakeSolver {
    fn solve(&mut self, batch: &[SolveReq]) -> Result<Vec<Vec<KeyAnswer>>, String> {
        let mut out = vec![];
        let mut labels = vec![];
        for r in batch {
            labels.push(r.label.clone());
            let d = r.digest();
            if let Some(a) = self.journal.get(&d) {
                out.push(a.clone());
                continue;
            }
            if self.fail_after.is_some_and(|n| self.fresh >= n) {
                return Err("interrupted".into());
            }
            self.fresh += 1;
            let a: Vec<KeyAnswer> = r.keys.iter().map(|k| answer(&self.oracle, r, k)).collect();
            self.journal.insert(d, a.clone());
            out.push(a);
        }
        self.batches.push(labels);
        Ok(out)
    }
}

fn solver(o: &Oracle) -> FakeSolver {
    FakeSolver {
        oracle: o.clone(),
        batches: vec![],
        fail_after: None,
        journal: BTreeMap::new(),
        fresh: 0,
    }
}

fn policy(over: &str) -> CoreSeedPolicy {
    let mut p: Value = serde_json::from_str(over).unwrap();
    if p["minimise"].is_null() {
        p["minimise"] = serde_json::json!({});
    }
    serde_json::from_value(p).unwrap()
}

fn key_report<'a>(
    r: &'a CoreReport,
    k: &[i32],
) -> &'a rustred_campaign_core::coreseed::machine::KeyReport {
    r.keys.iter().find(|x| x.key == k).unwrap()
}

/// The decision record (cost members removed): what must not depend on CPUs or on resume.
fn decisions(r: &CoreReport) -> Value {
    let mut v = serde_json::to_value(r).unwrap();
    for g in v["groups"].as_array_mut().unwrap() {
        for p in g["pieces"].as_array_mut().unwrap() {
            p.as_object_mut().unwrap().remove("minimise_cost");
        }
    }
    v
}

fn rel(rhs: &[[i32; 4]]) -> Vec<Key> {
    rhs.iter().map(|x| x.to_vec()).collect()
}

const A: [i32; 4] = [2, 1, -1, 0];
const B: [i32; 4] = [3, 1, -2, 0];

#[test]
fn a_list_minimised_at_one_key_is_not_accepted_where_it_fails() {
    // A needs seed o1, B needs seed o2: the full rule list has both; minimised at B (highest dots+rank) it keeps only
    // o2 and FAILS at A: A is not in that list's domain; A gets its own list (second piece)
    let o1 = vec![1, 0, 0, 0];
    let o2 = vec![0, 0, 1, 0];
    let mut o = Oracle::default();
    o.opts.insert(
        A.to_vec(),
        vec![Opt {
            needs: vec![o1.clone()],
            rhs: rel(&[[1, 1, -1, 0]]),
            d0: D0Verdict::Regular,
        }],
    );
    o.opts.insert(
        B.to_vec(),
        vec![Opt {
            needs: vec![o2.clone()],
            rhs: rel(&[[2, 1, -2, 0]]),
            d0: D0Verdict::Regular,
        }],
    );
    let p = policy(r#"{"centre_scan": {"enabled": false}}"#);
    let mut s = solver(&o);
    let r = solve_core(&[A.to_vec(), B.to_vec()], &LINES, &p, &mut s).unwrap();
    let g = &r.groups[0];
    assert_eq!(g.pieces.len(), 2, "{:#?}", g.pieces);
    assert_eq!(g.pieces[0].domain, vec![B.to_vec()]);
    assert_eq!(g.pieces[0].failed, vec![A.to_vec()]);
    assert_eq!(g.pieces[0].list, vec![o2.clone()]);
    assert_eq!(g.pieces[0].minimise["domain_tested"]["solved"], 1);
    assert_eq!(g.pieces[1].domain, vec![A.to_vec()]);
    assert_eq!(g.pieces[1].list, vec![o1.clone()]);
    // A's record shows the failed domain test of B's list
    let ka = key_report(&r, &A);
    assert!(
        ka.steps
            .iter()
            .any(|x| x.phase == "domain_test" && x.outcome == "no_relation")
    );
    assert_eq!(ka.status, "relation");
    assert_eq!(r.totals["status"]["relation"], 2);
}

#[test]
fn the_escalation_order_is_followed_and_recorded() {
    // C needs +2 on line 0 (only in S2 around the key); D needs line 0 three steps down (only around the shifted
    // centre "line 0 one step down" with Sm2); E has no relation anywhere
    let c = [2, 1, -1, 0];
    let d = [4, 1, -1, 0];
    let e = [3, 1, -1, 0];
    let mut o = Oracle::default();
    o.opts.insert(
        c.to_vec(),
        vec![Opt {
            needs: vec![vec![2, 0, 0, 0]],
            rhs: rel(&[[1, 1, -1, 0]]),
            d0: D0Verdict::Regular,
        }],
    );
    o.opts.insert(
        d.to_vec(),
        vec![Opt {
            needs: vec![vec![-3, 0, 0, 0]],
            rhs: rel(&[[1, 1, -1, 0]]),
            d0: D0Verdict::Regular,
        }],
    );
    let p = policy(r#"{"centre_scan": {"enabled": false}}"#);
    let mut s = solver(&o);
    let r = solve_core(&[c.to_vec(), d.to_vec(), e.to_vec()], &LINES, &p, &mut s).unwrap();
    let kc = key_report(&r, &c);
    assert_eq!(
        kc.first_solved_by.as_deref(),
        Some("escalation:escalation:shells_S2")
    );
    assert_eq!(kc.steps[0].phase, "rule");
    assert_eq!(kc.steps[0].outcome, "no_relation");
    let kd = key_report(&r, &d);
    assert_eq!(
        kd.first_solved_by.as_deref(),
        Some("escalation:escalation:shifted_centres:line0_down")
    );
    let phases: Vec<(String, String)> = kd
        .steps
        .iter()
        .map(|x| (x.rule.clone(), x.outcome.clone()))
        .collect();
    assert_eq!(phases[0], ("rule".into(), "no_relation".into()));
    assert_eq!(
        phases[1],
        ("escalation:shells_S2".into(), "no_relation".into())
    );
    assert_eq!(
        phases[2],
        ("escalation:shells_S2m1".into(), "no_relation".into())
    );
    assert_eq!(
        phases[3],
        ("escalation:shells_S2_S2m1".into(), "no_relation".into())
    );
    assert!(
        phases
            .iter()
            .any(|x| x.0 == "escalation:shifted_centres:line0_down" && x.1 == "relation")
    );
    let ke = key_report(&r, &e);
    assert_eq!(ke.status, "no_relation_decided");
    assert!(ke.steps.len() >= 3 && ke.steps.iter().all(|x| x.outcome == "no_relation"));
}

#[test]
fn a_budget_stop_is_never_no_relation() {
    let e = [3, 1, -1, 0];
    let f = [2, 1, -2, 0];
    let mut o = Oracle::default();
    o.budget.insert(e.to_vec());
    // F: budget only for the lists larger than the rule's (escalation), no relation in the rule's list: undecided
    let p0 = CoreSeedPolicy::default();
    let rule_list = rustred_campaign_core::coreseed::seeds::rule_offsets(
        &[f.to_vec()],
        &LINES,
        &p0.rule.centres,
        &p0.rule.shells,
        &f,
    )
    .unwrap();
    o.budget_above.insert(f.to_vec(), rule_list.len());
    let p = policy(r#"{"centre_scan": {"enabled": false}}"#);
    let mut s = solver(&o);
    let r = solve_core(&[e.to_vec(), f.to_vec()], &LINES, &p, &mut s).unwrap();
    let ke = key_report(&r, &e);
    assert_eq!(ke.status, "undecided");
    assert!(ke.steps.iter().all(|x| x.outcome == "budget"));
    let kf = key_report(&r, &f);
    assert!(
        kf.steps.iter().any(|x| x.outcome == "no_relation"),
        "{:?}",
        kf.steps
    );
    assert!(
        kf.steps.iter().any(|x| x.outcome == "budget"),
        "{:?}",
        kf.steps
    );
    assert_eq!(kf.status, "undecided");
    assert_eq!(r.totals["status"]["undecided"], 2);
    assert!(r.totals["status"]["no_relation_decided"].is_null());
}

fn mixed_oracle() -> Oracle {
    let mut o = Oracle::default();
    let ks: Vec<[i32; 4]> = vec![
        [2, 1, -1, 0],
        [3, 1, -1, 0],
        [2, 1, -2, 0],
        [3, 1, -2, 0],
        [4, 1, -1, 0],
    ];
    for (i, k) in ks.iter().enumerate() {
        let needs = match i {
            0 => vec![vec![1, 0, 0, 0], vec![0, 0, 1, 0]],
            1 => vec![vec![0, 0, 1, 0], vec![-1, 0, 0, 0]],
            2 => vec![vec![2, 0, 0, 0]],
            3 => vec![vec![0, 0, 1, 0], vec![1, 0, 0, 0]],
            _ => vec![vec![-3, 0, 0, 0]],
        };
        o.opts.insert(
            k.to_vec(),
            vec![Opt {
                needs,
                rhs: rel(&[[1, 1, -1, 0], [1, 1, 0, 0]]),
                d0: D0Verdict::Regular,
            }],
        );
    }
    o.opts.insert(vec![3, 1, -3, 0], vec![]);
    o.budget.insert(vec![2, 1, -3, 0]);
    o
}

fn mixed_keys() -> Vec<Key> {
    vec![
        vec![2, 1, -1, 0],
        vec![3, 1, -1, 0],
        vec![2, 1, -2, 0],
        vec![3, 1, -2, 0],
        vec![4, 1, -1, 0],
        vec![3, 1, -3, 0],
        vec![2, 1, -3, 0],
    ]
}

#[test]
fn resume_mid_escalation_gives_the_same_record() {
    let o = mixed_oracle();
    let p = policy("{}");
    let mut full = solver(&o);
    let r_full = solve_core(&mixed_keys(), &LINES, &p, &mut full).unwrap();
    let n = full.fresh;
    assert!(n > 6, "{n} requests");
    for cut in [2, n / 2, n - 1] {
        let mut s1 = solver(&o);
        s1.fail_after = Some(cut);
        assert!(solve_core(&mixed_keys(), &LINES, &p, &mut s1).is_err());
        // restart with the journal of the interrupted run
        let mut s2 = solver(&o);
        s2.journal = s1.journal.clone();
        let r2 = solve_core(&mixed_keys(), &LINES, &p, &mut s2).unwrap();
        assert_eq!(decisions(&r2), decisions(&r_full), "cut after {cut}");
        assert_eq!(
            s2.fresh,
            n - cut,
            "only the missing requests are solved again"
        );
    }
}

#[test]
fn results_do_not_depend_on_the_number_of_parallel_trials() {
    let o = mixed_oracle();
    let r1 = solve_core(
        &mixed_keys(),
        &LINES,
        &policy(r#"{"minimise": {"parallel_trials": 1}}"#),
        &mut solver(&o),
    )
    .unwrap();
    let r4 = solve_core(
        &mixed_keys(),
        &LINES,
        &policy(r#"{"minimise": {"parallel_trials": 4}}"#),
        &mut solver(&o),
    )
    .unwrap();
    let r9 = solve_core(
        &mixed_keys(),
        &LINES,
        &policy(r#"{"minimise": {"parallel_trials": 9}}"#),
        &mut solver(&o),
    )
    .unwrap();
    let strip = |r: &CoreReport| {
        let mut v = decisions(r);
        v.as_object_mut().unwrap().remove("policy_digest");
        v["totals"].as_object_mut().unwrap().remove("requests");
        v
    };
    assert_eq!(strip(&r1), strip(&r4));
    assert_eq!(strip(&r1), strip(&r9));
}

#[test]
fn regular_preferred_tries_the_up_centre_and_keeps_both_alternatives() {
    // G: in the rule's list only a SINGULAR relation (seed o_s); a regular one needs +2 on line 0 = the up-centre
    // (line 0 at its lowest power, one step up) with S1
    let g = [2, 1, -1, 0];
    let h = [3, 1, -1, 0];
    let o_s = vec![0, 0, 1, 0];
    let mut o = Oracle::default();
    o.opts.insert(
        g.to_vec(),
        vec![
            Opt {
                needs: vec![vec![2, 0, 0, 0]],
                rhs: rel(&[[1, 1, -1, 0], [1, 1, 0, 0], [2, 1, 0, 0]]),
                d0: D0Verdict::Regular,
            },
            Opt {
                needs: vec![o_s.clone()],
                rhs: rel(&[[1, 1, -1, 0]]),
                d0: D0Verdict::Singular { order: Some(1) },
            },
        ],
    );
    o.opts.insert(
        h.to_vec(),
        vec![Opt {
            needs: vec![o_s.clone()],
            rhs: rel(&[[1, 1, -1, 0]]),
            d0: D0Verdict::Regular,
        }],
    );
    let ks = vec![g.to_vec(), h.to_vec()];
    let r = solve_core(&ks, &LINES, &policy("{}"), &mut solver(&o)).unwrap();
    let kg = key_report(&r, &g);
    assert!(kg.alternatives.len() >= 2);
    let ch = &kg.alternatives[kg.chosen.unwrap()];
    assert_eq!(ch.d0, D0Verdict::Regular);
    assert!(ch.rule.contains("line0_up"), "{}", ch.rule);
    assert!(
        kg.alternatives
            .iter()
            .any(|a| matches!(a.d0, D0Verdict::Singular { .. }))
    );
    assert_eq!(kg.first_solved_by.as_deref(), Some("rule:rule"));
    // mode any_relation: no regular search, the smallest relation (the singular one) is chosen
    let ra = solve_core(
        &ks,
        &LINES,
        &policy(r#"{"mode": "any_relation", "centre_scan": {"enabled": false}}"#),
        &mut solver(&o),
    )
    .unwrap();
    let ka = key_report(&ra, &g);
    assert_eq!(ka.alternatives.len(), 1);
    assert!(matches!(ka.alternatives[0].d0, D0Verdict::Singular { .. }));
    // the split: G's chosen rule (up-centre) and H's (the rule) are different pieces
    let pieces = &r.groups[0].pieces;
    assert!(
        pieces
            .iter()
            .any(|p| p.domain == vec![g.to_vec()] && p.rule.contains("line0_up"))
    );
    assert!(
        pieces
            .iter()
            .any(|p| p.domain.contains(&h.to_vec()) && p.rule == "rule")
    );
}

#[test]
fn a_large_relation_triggers_the_centre_scan_and_the_smaller_wins() {
    // K: the rule gives a 5-term relation; the centre "line 0 one step down" a 2-term one; min_terms 3
    let k = [4, 1, -1, 0];
    let mut o = Oracle::default();
    o.opts.insert(
        k.to_vec(),
        vec![
            Opt {
                needs: vec![vec![-3, 0, 0, 0]],
                rhs: rel(&[[1, 1, -1, 0], [2, 1, 0, 0]]),
                d0: D0Verdict::Regular,
            },
            Opt {
                needs: vec![vec![0, 0, 1, 0]],
                rhs: rel(&[
                    [1, 1, -1, 0],
                    [2, 1, -1, 0],
                    [1, 1, 0, 0],
                    [2, 1, 0, 0],
                    [3, 1, 0, 0],
                ]),
                d0: D0Verdict::Regular,
            },
        ],
    );
    let r = solve_core(
        &[k.to_vec()],
        &LINES,
        &policy(r#"{"centre_scan": {"min_terms": 3}}"#),
        &mut solver(&o),
    )
    .unwrap();
    let kr = key_report(&r, &k);
    let ch = &kr.alternatives[kr.chosen.unwrap()];
    assert_eq!(ch.rhs_terms, 2);
    assert!(
        ch.rule.starts_with("centre_scan:scan:line0_down@S0+"),
        "{}",
        ch.rule
    );
    // without the trigger the scan does not run
    let r0 = solve_core(
        &[k.to_vec()],
        &LINES,
        &policy(r#"{"centre_scan": {"min_terms": 9}}"#),
        &mut solver(&o),
    )
    .unwrap();
    assert_eq!(key_report(&r0, &k).alternatives.len(), 1);
}

// The engine-minimiser / emulation interchangeability test moved to lane seedmin with the minimiser (one minimiser in
// the tree: coreseed calls `seedmin::bridge::minimise`).
