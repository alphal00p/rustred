//! Lane coreseed (interfaces/coreseed.md 4): the IBP-core flow through REAL child processes (`rr_fake_child` emulating
//! kind solve-recipe, its script written per job from a data oracle) and the closing loop's ACCEPT CHAIN on the toy
//! node of the termmin suites (chart 110: two lines, one numerator; the table serves only I(5,1,0)). Spawns processes:
//! run ONLY inside a private pid namespace (`LIGHT_PIDNS=1 .../light_build.sh coreseed test -p rustred-campaign --test
//! coreseed_fake`).

mod sup_common;
mod termmin_support;

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::coreseed::{ChildSolver, CoreDriverConfig, FakeCore, FakeOpt};
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_core::coreseed::spec::CoreSeedPolicy;
use rustred_campaign_core::coreseed::{
    CoreReport, CoreSolver, D0Verdict, Key, KeyAnswer, RegularityJudge, SolveReq, solve_core,
};
use rustred_campaign_core::import::{ImportedSet, TableSetSpec};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use sup_common::*;
use termmin_support::*;

const LINES: [bool; 3] = [true, true, false];
/// A right-hand-side key that marks a relation SINGULAR for the test judge (sub-sector: always lower).
const SING: [i32; 3] = [9, 0, 0];

struct TestJudge;
impl RegularityJudge for TestJudge {
    fn is_regular_at(&self, record: &Value, _key: &[i32], _d0: &str) -> D0Verdict {
        let marker = format!("I({},{},{})", SING[0], SING[1], SING[2]);
        let sing = record["rhs"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| t[0].as_str() == Some(marker.as_str()));
        if sing {
            D0Verdict::Singular { order: Some(1) }
        } else {
            D0Verdict::Regular
        }
    }
}

fn load_set(files: &TmFiles) -> ImportedSet {
    let spec = TableSetSpec::read(&files.dir.join("import-spec.json")).unwrap();
    rustred_campaign_core::import::load(&spec, Path::new("/")).unwrap()
}

fn sampler() -> Arc<Sampler> {
    Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(200),
            collect_foreign_pinning: false,
        })
        .unwrap(),
    )
}

fn config(tmp: &Path, files: &TmFiles, fake: FakeCore, journal: &Path) -> CoreDriverConfig {
    CoreDriverConfig {
        node: "toy".into(),
        program: Program::from_file("rr_fake_child", Path::new(FAKE), Invocation::Contract)
            .unwrap(),
        family: files.dir.join("TOY.family"),
        order: "toy-order".into(),
        cpus: cpus()
            .split(',')
            .filter_map(|x| x.trim().parse().ok())
            .take(3)
            .collect(),
        pin: true,
        work_root: tmp.join("work"),
        journal: journal.to_path_buf(),
        keys_per_job: 2,
        cap_bytes: 268_435_456,
        key_budget_s: Some(60),
        max_trace_rows: None,
        wall_cap_s: 120,
        licence_env: None,
        d0: "4".into(),
        keep_trial_outputs: false,
        verbose: false,
        fake: Some(fake),
        cpu_budget_s: None,
    }
}

fn opt(needs: &[[i32; 3]], rhs: &[[i32; 3]]) -> FakeOpt {
    FakeOpt {
        needs: needs.iter().map(|x| x.to_vec()).collect(),
        rhs: rhs.iter().map(|x| x.to_vec()).collect(),
    }
}

fn fake(entries: &[([i32; 3], Vec<FakeOpt>)], budget: &[[i32; 3]]) -> FakeCore {
    FakeCore {
        opts: entries
            .iter()
            .map(|(k, o)| (format!("{:?}", k.to_vec()), o.clone()))
            .collect(),
        budget: budget.iter().map(|k| format!("{:?}", k.to_vec())).collect(),
        ms: 1,
    }
}

fn run(tag: &str, f: FakeCore, keys: &[[i32; 3]], policy: &str) -> (CoreReport, u64) {
    let tmp = Tmp::new(tag);
    let files = toy(&tmp.path.join("material"), &TmToy::default());
    let mut set = load_set(&files);
    let journal = tmp.path.join("out/journal.jsonl");
    let p: CoreSeedPolicy = serde_json::from_str(policy).unwrap();
    let mut s = ChildSolver::new(
        config(&tmp.path, &files, f, &journal),
        &mut set,
        Box::new(TestJudge),
        sampler(),
    )
    .unwrap();
    let ks: Vec<Key> = keys.iter().map(|k| k.to_vec()).collect();
    let r = solve_core(&ks, &LINES, &p, &mut s).unwrap();
    (r, s.requests_solved)
}

fn key<'a>(
    r: &'a CoreReport,
    k: &[i32],
) -> &'a rustred_campaign_core::coreseed::machine::KeyReport {
    r.keys.iter().find(|x| x.key == k).unwrap()
}

const A: [i32; 3] = [2, 1, -1];
const B: [i32; 3] = [3, 1, -2];
/// sub-sector right-hand sides (always strictly lower under the node comparator)
const R1: [i32; 3] = [1, 0, 0];
const R2: [i32; 3] = [2, 0, -1];

#[test]
fn overfit_a_list_good_at_one_key_and_failing_at_another_is_not_accepted_there() {
    let f = fake(
        &[
            (A, vec![opt(&[[1, 0, 0]], &[R1])]),
            (B, vec![opt(&[[0, 0, 1]], &[R2])]),
        ],
        &[],
    );
    let (r, _) = run(
        "cs-overfit",
        f,
        &[A, B],
        r#"{"centre_scan": {"enabled": false}}"#,
    );
    let g = &r.groups[0];
    assert_eq!(g.pieces.len(), 2, "{:#?}", g.pieces);
    assert_eq!(g.pieces[0].domain, vec![B.to_vec()]);
    assert_eq!(g.pieces[0].failed, vec![A.to_vec()]);
    assert_eq!(g.pieces[0].list, vec![vec![0, 0, 1]]);
    assert_eq!(g.pieces[1].domain, vec![A.to_vec()]);
    assert!(
        key(&r, &A)
            .steps
            .iter()
            .any(|s| s.phase == "domain_test" && s.outcome == "no_relation")
    );
    // every accepted relation passed the accept chain (exact proof flag, descent, binding)
    assert!(r.keys.iter().all(|k| k.status == "relation"));
}

#[test]
fn escalation_order_is_followed_and_recorded() {
    let c = [2, 1, -1];
    let d = [4, 1, -1];
    let e = [3, 1, -1];
    let f = fake(
        &[
            (c, vec![opt(&[[2, 0, 0]], &[R1])]),
            (d, vec![opt(&[[-3, 0, 0]], &[R1])]),
        ],
        &[],
    );
    let (r, _) = run(
        "cs-escal",
        f,
        &[c, d, e],
        r#"{"centre_scan": {"enabled": false}}"#,
    );
    let kc = key(&r, &c);
    assert_eq!(
        kc.first_solved_by.as_deref(),
        Some("escalation:escalation:shells_S2")
    );
    assert_eq!(
        (kc.steps[0].phase.as_str(), kc.steps[0].outcome.as_str()),
        ("rule", "no_relation")
    );
    let kd = key(&r, &d);
    assert_eq!(
        kd.first_solved_by.as_deref(),
        Some("escalation:escalation:shifted_centres:line0_down")
    );
    let rules: Vec<&str> = kd.steps.iter().map(|s| s.rule.as_str()).collect();
    assert_eq!(
        &rules[..4],
        &[
            "rule",
            "escalation:shells_S2",
            "escalation:shells_S2m1",
            "escalation:shells_S2_S2m1"
        ]
    );
    assert_eq!(key(&r, &e).status, "no_relation_decided");
}

#[test]
fn a_budget_stop_is_not_no_relation() {
    let e = [3, 1, -1];
    let f = fake(&[], &[e]);
    let (r, _) = run(
        "cs-budget",
        f,
        &[e, A],
        r#"{"centre_scan": {"enabled": false}}"#,
    );
    let ke = key(&r, &e);
    assert_eq!(ke.status, "undecided");
    assert!(
        ke.steps.iter().all(|s| s.outcome == "budget"),
        "{:?}",
        ke.steps
    );
    assert_eq!(key(&r, &A).status, "no_relation_decided");
}

#[test]
fn regular_preferred_keeps_both_alternatives_through_children() {
    // A: in the rule's list only a singular relation; the up-centre (line 0 at its lowest power one step up) gives a
    // regular one
    let f = fake(
        &[
            (
                A,
                vec![opt(&[[2, 0, 0]], &[R1, R2]), opt(&[[0, 0, 1]], &[SING])],
            ),
            (B, vec![opt(&[[0, 0, 1]], &[R2])]),
        ],
        &[],
    );
    let (r, _) = run("cs-regular", f, &[A, B], "{}");
    let ka = key(&r, &A);
    let ch = &ka.alternatives[ka.chosen.unwrap()];
    assert_eq!(ch.d0, D0Verdict::Regular);
    assert!(
        ka.alternatives
            .iter()
            .any(|a| matches!(a.d0, D0Verdict::Singular { .. }))
    );
}

/// A solver that stops after `n` fresh batches (an interrupted driver).
struct Cut<'a, 'b> {
    inner: &'a mut ChildSolver<'b>,
    left: usize,
}
impl CoreSolver for Cut<'_, '_> {
    fn solve(&mut self, b: &[SolveReq]) -> Result<Vec<Vec<KeyAnswer>>, String> {
        if self.left == 0 {
            return Err("interrupted".into());
        }
        self.left -= 1;
        self.inner.solve(b)
    }
}

fn decisions(r: &CoreReport) -> Value {
    let mut v = serde_json::to_value(r).unwrap();
    for g in v["groups"].as_array_mut().unwrap() {
        for p in g["pieces"].as_array_mut().unwrap() {
            p.as_object_mut().unwrap().remove("minimise_cost");
        }
    }
    v
}

#[test]
fn resume_mid_escalation_gives_the_same_record() {
    let c = [2, 1, -1];
    let d = [4, 1, -1];
    let mk = || {
        fake(
            &[
                (c, vec![opt(&[[2, 0, 0]], &[R1])]),
                (d, vec![opt(&[[-3, 0, 0]], &[R1])]),
                (B, vec![opt(&[[0, 0, 1], [1, 0, 0]], &[R2])]),
            ],
            &[[3, 1, -1]],
        )
    };
    let keys = [c, d, B, [3, 1, -1]];
    let pol: CoreSeedPolicy =
        serde_json::from_str(r#"{"centre_scan": {"enabled": false}}"#).unwrap();
    let (full, n_full) = run(
        "cs-resume-full",
        mk(),
        &keys,
        r#"{"centre_scan": {"enabled": false}}"#,
    );
    // interrupted after 3 batches, then resumed from the journal in a NEW driver
    let tmp = Tmp::new("cs-resume");
    let files = toy(&tmp.path.join("material"), &TmToy::default());
    let journal: PathBuf = tmp.path.join("out/journal.jsonl");
    let ks: Vec<Key> = keys.iter().map(|k| k.to_vec()).collect();
    let first = {
        let mut set = load_set(&files);
        let mut s = ChildSolver::new(
            config(&tmp.path, &files, mk(), &journal),
            &mut set,
            Box::new(TestJudge),
            sampler(),
        )
        .unwrap();
        let mut cut = Cut {
            inner: &mut s,
            left: 3,
        };
        assert!(solve_core(&ks, &LINES, &pol, &mut cut).is_err());
        s.requests_solved
    };
    let mut set = load_set(&files);
    let mut s = ChildSolver::new(
        config(&tmp.path, &files, mk(), &journal),
        &mut set,
        Box::new(TestJudge),
        sampler(),
    )
    .unwrap();
    assert_eq!(s.requests_journaled, first);
    let again = solve_core(&ks, &LINES, &pol, &mut s).unwrap();
    assert_eq!(decisions(&again), decisions(&full));
    assert_eq!(
        first + s.requests_solved,
        n_full,
        "only the missing requests ran again"
    );
    let _ = BTreeMap::<u8, u8>::new();
}
