//! Lane polefix (audit r2-poles F10): the seed minimiser's child solver (closeloop::seedmin::ChildTrialSolver) with FAKE
//! children through the REAL launcher.
//! 1. `pole_objective: off`: NO `pole-regular-at` job at all, even when one is configured.
//! 2. CLOSE FIRST: a solved trial is journaled when it is SOLVED, before the pole job answers: a pole job that dies
//!    loses nothing; a resumed solver answers the trial from the journal (no solve job), asks the pole job again for
//!    the orders still undecided and returns the same answers as an uninterrupted run.
//!
//! Spawns processes: run ONLY under `LIGHT_PIDNS=1` (private pid namespace, rule L10).

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::coreseed::PolesCfg;
use rustred_campaign::closeloop::seedmin::{ChildConfig, ChildTrialSolver, FakeAlt, FakeOracle};
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::fake::{FakeOp, FakeScript, Fault, FaultKind, OnSigterm, RssPlan};
use rustred_campaign_core::seedmin::{
    EngineCaps, PoleOrder, StubRelationPoles, Trial, TrialAnswer, TrialMode, TrialSolver,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const FAKE: &str = env!("CARGO_BIN_EXE_rr_fake_child");
const ORDER: &str =
    r#"{"name":"o","slot_permutation":null,"coordinate_priority":null,"source_order":"Input"}"#;

/// A NEW directory under the (private, per-invocation) TMPDIR; removed at the end of the test only (L18).
fn root(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let p = std::env::temp_dir().join(format!("rr-polefix-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&p).unwrap();
    p
}

fn program() -> Program {
    let mut p = Program::from_file("rr_fake_child", Path::new(FAKE), Invocation::Contract).unwrap();
    p.features_required = vec!["fake".into()];
    p
}

/// ka's relation names I(9,..) (the fake pole child's rule: order 1), kb's does not (order 0).
fn oracle() -> FakeOracle {
    let mut o = FakeOracle {
        ms: 5,
        ..Default::default()
    };
    o.alts.insert(
        vec![3, 2, -1],
        vec![FakeAlt {
            needs: vec![vec![0, 0, 0]],
            rhs: vec![vec![9, 1, -1]],
        }],
    );
    o.alts.insert(
        vec![2, 3, -1],
        vec![FakeAlt {
            needs: vec![vec![0, 0, 0]],
            rhs: vec![vec![2, 2, -1]],
        }],
    );
    o
}

fn poles(faults: Vec<Fault>) -> PolesCfg {
    PolesCfg {
        program: program(),
        threads: 1,
        cap_bytes: 256 << 20,
        wall_cap_s: 60,
        fake: Some(FakeScript {
            script: vec![FakeOp::Prepare { ms: 1 }],
            on_sigterm: OnSigterm::Graceful,
            slow_ms: 0,
            start_delay_ms: 0,
            rss: RssPlan::default(),
            helpers: vec![],
            faults,
            time_scale: 1.0,
            volatile_jitter: false,
        }),
    }
}

fn config(dir: &Path, poles: Option<PolesCfg>, objective: &str) -> ChildConfig {
    let family = dir.join("family.sym");
    std::fs::write(&family, "fake family\n").unwrap();
    ChildConfig {
        program: program(),
        family,
        order: ORDER.into(),
        lines: vec![true, true, false],
        domain: None,
        relation_rule: "first_admissible".into(),
        cpus: vec![0, 1],
        pin_cpus: false,
        threads_per_job: 2,
        cap_bytes: 256 << 20,
        key_budget_s: None,
        max_trace_rows: None,
        wall_cap_s: 60,
        work_root: dir.join("work"),
        licence_env: None,
        keep_run_dirs: false,
        d0: "4".into(),
        records_out: None,
        records_cap_bytes: 0,
        fake: Some(oracle()),
        poles,
        pole_objective: objective.into(),
        verbose: false,
    }
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

fn solver(cfg: ChildConfig) -> ChildTrialSolver {
    ChildTrialSolver::new(
        cfg,
        sampler(),
        Box::new(StubRelationPoles),
        EngineCaps::default(),
    )
    .unwrap()
}

/// One trial: the key list at centre zero with the list {0} (both keys solve).
fn trials() -> Vec<Trial> {
    vec![Trial {
        label: "t0".into(),
        centre: vec![0, 0, 0],
        list: vec![vec![0, 0, 0], vec![1, 0, 0]],
        keys: vec![vec![3, 2, -1], vec![2, 3, -1]],
        mode: TrialMode::Solve,
        engine_minimise: None,
    }]
}

fn poles_of(a: &[Vec<TrialAnswer>]) -> Vec<PoleOrder> {
    a[0].iter()
        .map(|x| x.relation().map(|f| f.pole).expect("a relation"))
        .collect()
}

fn lines(p: &Path) -> usize {
    std::fs::read_to_string(p)
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

#[test]
fn no_pole_job_when_the_objective_is_off() {
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run under LIGHT_PIDNS=1 (private pid namespace, rule L10)"
    );
    let dir = root("off");
    let mut s = solver(config(&dir, Some(poles(vec![])), "off"));
    let a = s.solve(&trials()).unwrap();
    assert_eq!(s.stats.pole_jobs, 0, "objective off: no pole job");
    assert_eq!(
        poles_of(&a),
        vec![PoleOrder::Undecided, PoleOrder::Undecided]
    );
    // on: the job runs and gives the orders
    let dir2 = root("on");
    let mut s2 = solver(config(&dir2, Some(poles(vec![])), "on"));
    let a2 = s2.solve(&trials()).unwrap();
    assert_eq!(s2.stats.pole_jobs, 1);
    assert_eq!(
        poles_of(&a2),
        vec![PoleOrder::Order { order: 1 }, PoleOrder::Order { order: 0 }]
    );
    // a bad word is refused
    assert!(
        ChildTrialSolver::new(
            config(&root("bad"), None, "maybe"),
            sampler(),
            Box::new(StubRelationPoles),
            EngineCaps::default()
        )
        .is_err()
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn a_solved_trial_is_journaled_before_the_pole_job_and_a_resume_asks_the_job_again() {
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run under LIGHT_PIDNS=1 (private pid namespace, rule L10)"
    );
    let dir = root("journal");
    let work = dir.join("work");
    // the pole job dies (exit without OUTCOME; no core dump): the trial is journaled anyway, its records wait in
    // poles_pending.jsonl
    let dying = vec![Fault {
        at_op: 0,
        kind: FaultKind::Exit,
        bytes: 0,
        code: 3,
        path: None,
    }];
    let mut s1 = solver(config(&dir, Some(poles(dying)), "on"));
    let a1 = s1.solve(&trials()).unwrap();
    assert_eq!(s1.stats.jobs, 1);
    assert_eq!(s1.stats.pole_jobs, 1);
    assert_eq!(
        poles_of(&a1),
        vec![PoleOrder::Undecided, PoleOrder::Undecided]
    );
    assert_eq!(
        lines(&work.join("journal.jsonl")),
        1,
        "journaled when solved"
    );
    assert_eq!(lines(&work.join("poles_pending.jsonl")), 2);
    drop(s1);
    // resume with a working pole job: no solve job, one pole job, the orders of an uninterrupted run
    let mut s2 = solver(config(&dir, Some(poles(vec![])), "on"));
    assert_eq!(s2.journal_len(), 1);
    let a2 = s2.solve(&trials()).unwrap();
    assert_eq!(s2.stats.jobs, 0, "the solve is never repeated");
    assert_eq!(
        s2.stats.pole_jobs, 1,
        "the undecided orders are asked again"
    );
    let want = vec![PoleOrder::Order { order: 1 }, PoleOrder::Order { order: 0 }];
    assert_eq!(poles_of(&a2), want);
    // the orders are journaled as a later line: a third solver answers everything from the journal
    assert_eq!(lines(&work.join("journal.jsonl")), 2);
    drop(s2);
    let mut s3 = solver(config(&dir, Some(poles(vec![])), "on"));
    let a3 = s3.solve(&trials()).unwrap();
    assert_eq!((s3.stats.jobs, s3.stats.pole_jobs), (0, 0));
    assert_eq!(poles_of(&a3), want);
    // an uninterrupted run gives the same answers
    let dir4 = root("journal-ref");
    let mut s4 = solver(config(&dir4, Some(poles(vec![])), "on"));
    let a4 = s4.solve(&trials()).unwrap();
    assert_eq!(
        serde_json::to_value(&a4).unwrap(),
        serde_json::to_value(&a3).unwrap()
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir4);
}

/// Lane polefix (audit r2-poles F4): the closing loop's node setting `pole_objective` (absent = default objective,
/// `"none"` = no `choice` annotation on SOLVED lines, an object = its measures; anything else refused at config load)
/// and the annotation itself (`accept::solved_choice`, written only when the setting is not "none"). Pure.
#[test]
fn the_node_pole_objective_setting_and_the_solved_choice() {
    use rustred_campaign::closeloop::accept::solved_choice;
    use rustred_campaign::closeloop::config::CloseLoopConfig;
    use rustred_campaign_core::poles::PoleObjective;
    let cfg = |extra: &str| {
        format!(
            r#"{{"schema":"rustred.campaign.closeloop.v1","nodes":[{{"node":"toy","import_spec":"/x.json",
            "solve":{{"program":"p","family":"/f","order":"o","relation_rule":"first_admissible","max_keys_per_job":1,
            "threads":1,"cap_bytes":1,"expected_peak_bytes":1,"heartbeat_s":1,"stop_grace_s":1}}{extra}}}]}}"#
        )
    };
    let dflt = CloseLoopConfig::from_json(&cfg("")).unwrap();
    assert_eq!(
        dflt.nodes[0].pole_objective_setting().unwrap(),
        Some(PoleObjective::default())
    );
    // absent: not serialised (config digests unchanged)
    assert!(
        !serde_json::to_string(&dflt.nodes[0])
            .unwrap()
            .contains("pole_objective")
    );
    let none = CloseLoopConfig::from_json(&cfg(r#","pole_objective":"none""#)).unwrap();
    assert_eq!(none.nodes[0].pole_objective_setting().unwrap(), None);
    let obj =
        CloseLoopConfig::from_json(&cfg(r#","pole_objective":{"measures":["size"]}"#)).unwrap();
    let o = obj.nodes[0].pole_objective_setting().unwrap().unwrap();
    assert!(CloseLoopConfig::from_json(&cfg(r#","pole_objective":"off""#)).is_err());
    // the annotation names the objective in force (one candidate, no profile: ranked by size)
    let c = solved_choice(&o, "rel-1", 7);
    assert_eq!(c["chosen"], "rel-1");
    assert_eq!(c["measures"], serde_json::json!(["size"]));
    assert_eq!(c["profile_used"], "none");
}
