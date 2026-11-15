//! Lane polefix (audit r2-poles F2; POLES_OBJECTIVE.md 1: CLOSE FIRST): `coreseed::solve_core` hands the close-first
//! report (A + B: every key served or decided) to `CoreSolver::publish` BEFORE any request of the variant phase (centre
//! scan, regular escalation) or the lists; the variant phase has its own budget, is resumable through the journal and
//! only ADDS alternatives. A variant phase that never returns does not delay the report.
//! Toy chart (arity 4: two lines, two numerators), in-memory oracle solver; nothing five-loop specific.

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::Duration;

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

type Oracle = BTreeMap<Key, Vec<Opt>>;

fn answer(o: &Oracle, req: &SolveReq, k: &Key) -> KeyAnswer {
    let list: Vec<Offset> = serde_json::from_value(req.recipe["seeds"]["centres"].clone()).unwrap();
    let valid: Vec<&Offset> = list
        .iter()
        .filter(|off| {
            k.iter()
                .zip(off.iter())
                .zip(LINES)
                .all(|((&a, &b), l)| if l { a + b >= 1 } else { a + b <= 0 })
        })
        .collect();
    for (i, op) in o.get(k).into_iter().flatten().enumerate() {
        if op.needs.iter().all(|n| valid.contains(&n)) {
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
                minimise: None,
                record_sha256: format!("{k:?}#{i}"),
            }));
        }
    }
    KeyAnswer::NoRelation
}

fn is_variant(req: &SolveReq) -> bool {
    req.label.contains("/centre_scan:") || req.label.contains("/regular:")
}

/// G: the rule's list gives only a SINGULAR relation; the up-centre (variant phase R) gives a regular one. H: regular
/// from the rule. (The data of coreseed_core's up-centre test.)
fn oracle() -> (Oracle, Key, Key) {
    let g = vec![2, 1, -1, 0];
    let h = vec![3, 1, -1, 0];
    let o_s = vec![0, 0, 1, 0];
    let mut o = Oracle::new();
    o.insert(
        g.clone(),
        vec![
            Opt {
                needs: vec![vec![2, 0, 0, 0]],
                rhs: vec![vec![1, 1, -1, 0], vec![1, 1, 0, 0], vec![2, 1, 0, 0]],
                d0: D0Verdict::Regular,
            },
            Opt {
                needs: vec![o_s.clone()],
                rhs: vec![vec![1, 1, -1, 0]],
                d0: D0Verdict::Singular { order: Some(1) },
            },
        ],
    );
    o.insert(
        h.clone(),
        vec![Opt {
            needs: vec![o_s],
            rhs: vec![vec![1, 1, -1, 0]],
            d0: D0Verdict::Regular,
        }],
    );
    (o, g, h)
}

fn policy(over: &str) -> CoreSeedPolicy {
    serde_json::from_str(over).unwrap()
}

fn key<'a>(
    r: &'a CoreReport,
    k: &[i32],
) -> &'a rustred_campaign_core::coreseed::machine::KeyReport {
    r.keys.iter().find(|x| x.key == k).unwrap()
}

/// A journaled oracle solver that records the publication and the order of events.
struct Plain {
    o: Oracle,
    journal: BTreeMap<String, Vec<KeyAnswer>>,
    published: Vec<CoreReport>,
    events: Vec<String>,
    /// stop with an error after this many FRESH (not journaled) requests
    fail_after: Option<usize>,
    fresh: usize,
}

impl Plain {
    fn new(o: &Oracle) -> Plain {
        Plain {
            o: o.clone(),
            journal: BTreeMap::new(),
            published: vec![],
            events: vec![],
            fail_after: None,
            fresh: 0,
        }
    }
}

impl CoreSolver for Plain {
    fn solve(&mut self, batch: &[SolveReq]) -> Result<Vec<Vec<KeyAnswer>>, String> {
        let mut out = vec![];
        for r in batch {
            self.events
                .push(if is_variant(r) { "variant" } else { "request" }.into());
            let d = r.digest();
            if let Some(a) = self.journal.get(&d) {
                out.push(a.clone());
                continue;
            }
            if self.fail_after.is_some_and(|n| self.fresh >= n) {
                return Err("interrupted".into());
            }
            self.fresh += 1;
            let a: Vec<KeyAnswer> = r.keys.iter().map(|k| answer(&self.o, r, k)).collect();
            self.journal.insert(d, a.clone());
            out.push(a);
        }
        Ok(out)
    }
    fn publish(&mut self, report: &CoreReport) {
        self.events.push("publish".into());
        self.published.push(report.clone());
    }
}

/// A solver whose variant requests NEVER return until the test releases them (then they fail).
struct Hung {
    o: Oracle,
    report: mpsc::Sender<CoreReport>,
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
}

impl CoreSolver for Hung {
    fn solve(&mut self, batch: &[SolveReq]) -> Result<Vec<Vec<KeyAnswer>>, String> {
        if batch.iter().any(is_variant) {
            let _ = self.entered.send(());
            // blocks until the test drops its sender: a variant phase that never returns by itself
            let _ = self.release.recv();
            return Err("variant phase cancelled".into());
        }
        Ok(batch
            .iter()
            .map(|r| r.keys.iter().map(|k| answer(&self.o, r, k)).collect())
            .collect())
    }
    fn publish(&mut self, report: &CoreReport) {
        let _ = self.report.send(report.clone());
    }
}

#[test]
fn a_variant_phase_that_never_returns_does_not_delay_the_report() {
    let (o, g, h) = oracle();
    let (rep_tx, rep_rx) = mpsc::channel();
    let (ent_tx, ent_rx) = mpsc::channel();
    let (rel_tx, rel_rx) = mpsc::channel::<()>();
    let keys = vec![g.clone(), h.clone()];
    let worker = std::thread::spawn(move || {
        let mut s = Hung {
            o,
            report: rep_tx,
            entered: ent_tx,
            release: rel_rx,
        };
        solve_core(&keys, &LINES, &policy("{}"), &mut s)
    });
    // the report arrives although the variant phase is hung
    let cf = rep_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("the close-first report must be published before the variant phase");
    ent_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("the variant phase was entered (and is hung) after the publication");
    assert_eq!(cf.stage, "close_first");
    for k in [&g, &h] {
        let kr = key(&cf, k);
        assert_eq!(
            kr.status, "relation",
            "{k:?} served by the close-first phases"
        );
        assert!(kr.chosen.is_some());
    }
    // G is served by its SINGULAR relation (closing first: a pole never holds a key open)
    let kg = key(&cf, &g);
    assert_eq!(kg.alternatives.len(), 1);
    assert!(matches!(kg.alternatives[0].d0, D0Verdict::Singular { .. }));
    assert_eq!(kg.first_solved_by.as_deref(), Some("rule:rule"));
    // the close-first pieces: the full list of the serving rule on its keys (a valid split recipe)
    let pieces = &cf.groups[0].pieces;
    assert!(pieces.iter().all(|p| p.kind == "full_list"));
    assert!(
        pieces
            .iter()
            .any(|p| p.rule == "rule" && p.domain.contains(&g) && p.domain.contains(&h))
    );
    // the variant phase is cancelled: its error comes back AFTER the publication; nothing published is lost
    drop(rel_tx);
    let r = worker.join().unwrap();
    assert!(
        r.is_err(),
        "the cancelled variant phase is reported as an error"
    );
}

#[test]
fn the_variant_phase_only_adds_and_the_publication_precedes_it() {
    let (o, g, h) = oracle();
    let mut s = Plain::new(&o);
    let fin = solve_core(&[g.clone(), h.clone()], &LINES, &policy("{}"), &mut s).unwrap();
    assert_eq!(s.published.len(), 1);
    let cf = &s.published[0];
    assert_eq!(
        (cf.stage.as_str(), fin.stage.as_str()),
        ("close_first", "final")
    );
    // every request before the publication is a close-first request; every variant request comes after it
    let p = s.events.iter().position(|e| e == "publish").unwrap();
    assert!(s.events[..p].iter().all(|e| e == "request"));
    assert!(s.events[p + 1..].iter().any(|e| e == "variant"));
    // only additions: status, first_solved_by, the close-first alternatives and steps (prefixes) are unchanged
    assert_eq!(cf.keys.len(), fin.keys.len());
    for (a, b) in cf.keys.iter().zip(&fin.keys) {
        assert_eq!(a.key, b.key);
        assert_eq!(a.status, b.status);
        assert_eq!(a.first_solved_by, b.first_solved_by);
        let da: Vec<&str> = a
            .alternatives
            .iter()
            .map(|x| x.relation_digest.as_str())
            .collect();
        let db: Vec<&str> = b
            .alternatives
            .iter()
            .map(|x| x.relation_digest.as_str())
            .collect();
        assert_eq!(&db[..da.len()], da.as_slice(), "{:?}", a.key);
        assert!(b.steps.len() >= a.steps.len());
        for (x, y) in a.steps.iter().zip(&b.steps) {
            assert_eq!(
                serde_json::to_value(x).unwrap(),
                serde_json::to_value(y).unwrap()
            );
        }
    }
    // the variant made G regular in the final report (the user's default: regular at d0 always wins)
    let kg = key(&fin, &g);
    assert_eq!(kg.alternatives[kg.chosen.unwrap()].d0, D0Verdict::Regular);
}

#[test]
fn the_variant_budget_is_its_own_and_never_unserves_a_key() {
    let (o, g, h) = oracle();
    // a zero variant budget: no variant request at all; the keys stay served by the close-first relations
    let mut s = Plain::new(&o);
    let fin = solve_core(
        &[g.clone(), h.clone()],
        &LINES,
        &policy(r#"{"variants": {"max_requests": 0}}"#),
        &mut s,
    )
    .unwrap();
    assert!(!s.events.iter().any(|e| e == "variant"));
    let kg = key(&fin, &g);
    assert_eq!(kg.status, "relation");
    assert!(matches!(
        kg.alternatives[kg.chosen.unwrap()].d0,
        D0Verdict::Singular { .. }
    ));
    assert!(
        kg.steps
            .iter()
            .any(|st| st.outcome == "budget" && st.rule == "variant_budget"),
        "the untried variant is recorded as a budget step"
    );
    // the default policy omits the budget from its canonical form (digests of existing policies unchanged)
    let d = CoreSeedPolicy::default();
    let v: Value = serde_json::to_value(&d).unwrap();
    assert!(v.get("variants").is_none());
    assert_eq!(
        policy(r#"{"variants": {"max_requests": null}}"#).digest(),
        d.digest()
    );
}

#[test]
fn an_interrupted_variant_phase_resumes_with_the_same_publication_and_result() {
    let (o, g, h) = oracle();
    let keys = vec![g.clone(), h.clone()];
    let mut full = Plain::new(&o);
    let r_full = solve_core(&keys, &LINES, &policy("{}"), &mut full).unwrap();
    let close_first_requests = full.events.iter().position(|e| e == "publish").unwrap();
    // interrupt at the first variant request
    let mut s1 = Plain::new(&o);
    s1.fail_after = Some(close_first_requests);
    assert!(solve_core(&keys, &LINES, &policy("{}"), &mut s1).is_err());
    assert_eq!(s1.published.len(), 1, "published before the interruption");
    // resume with the journal: the same publication, then the same final report
    let mut s2 = Plain::new(&o);
    s2.journal = s1.journal.clone();
    let r2 = solve_core(&keys, &LINES, &policy("{}"), &mut s2).unwrap();
    // the decision record (cost members removed, as coreseed_core's resume test)
    let j = |r: &CoreReport| {
        let mut v = serde_json::to_value(r).unwrap();
        for g in v["groups"].as_array_mut().unwrap() {
            for p in g["pieces"].as_array_mut().unwrap() {
                p.as_object_mut().unwrap().remove("minimise_cost");
            }
        }
        v
    };
    assert_eq!(j(&s1.published[0]), j(&s2.published[0]));
    assert_eq!(j(&r_full), j(&r2));
}
