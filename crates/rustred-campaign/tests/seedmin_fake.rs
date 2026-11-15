//! Lane seedmin: the supervisor's child solver with FAKE children (`rr_fake_child` emulating kind solve-recipe) through
//! the REAL launcher: joint minimisation, every trial one child job, journal resume (a second run answers every trial
//! from the journal and launches nothing), records accepted by the adapter only. Spawns processes: run ONLY under
//! `LIGHT_PIDNS=1` (private pid namespace, rule L10):
//! `LIGHT_PIDNS=1 light_build.sh seedmin test -p rustred-campaign --test seedmin_fake`

use rustred_campaign::adapters::job::{Invocation, Program};
use rustred_campaign::closeloop::seedmin::{
    ChildConfig, ChildTrialSolver, FakeAlt, FakeOracle, engine_caps_from_manifest,
};
use rustred_campaign::resources::{Sampler, SamplerConfig};
use rustred_campaign_core::seedmin::{
    EngineCaps, LocalValuationJudge, PoleOrder, SeedMinError, SeedMinPolicy, StubSystemPoles,
    minimise_joint,
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
    let p = std::env::temp_dir().join(format!("rr-seedmin-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&p).unwrap();
    p
}

fn config(dir: &Path, oracle: FakeOracle) -> ChildConfig {
    let family = dir.join("family.sym");
    std::fs::write(&family, "fake family\n").unwrap();
    let mut program =
        Program::from_file("rr_fake_child", Path::new(FAKE), Invocation::Contract).unwrap();
    program.features_required = vec!["fake".into()];
    ChildConfig {
        program,
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
        records_out: Some(dir.join("relations.jsonl")),
        records_cap_bytes: 1 << 20,
        fake: Some(oracle),
        poles: None,
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

#[test]
fn joint_minimisation_through_fake_children_resumes_from_the_journal() {
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run under LIGHT_PIDNS=1 (private pid namespace, rule L10)"
    );
    let dir = root("joint");
    let ka = vec![3, 2, -1];
    let kb = vec![2, 3, -1];
    let mut oracle = FakeOracle {
        ms: 5,
        ..Default::default()
    };
    // A needs the key and one line-0 step; B the key and one numerator step (offsets from the key = centre zero)
    oracle.alts.insert(
        ka.clone(),
        vec![FakeAlt {
            needs: vec![vec![0, 0, 0], vec![1, 0, 0]],
            rhs: vec![vec![2, 2, -1]],
        }],
    );
    oracle.alts.insert(
        kb.clone(),
        vec![FakeAlt {
            needs: vec![vec![0, 0, 0], vec![0, 0, -1]],
            rhs: vec![vec![2, 2, -1], vec![1, 3, -1]],
        }],
    );
    let list: Vec<Vec<i32>> = vec![
        vec![0, 0, 0],
        vec![1, 0, 0],
        vec![0, 1, 0],
        vec![0, 0, -1],
        vec![1, 1, 0],
        vec![-1, 0, 0],
    ];
    let policy = SeedMinPolicy {
        parallel_trials: 2,
        ..SeedMinPolicy::default()
    };
    let keys = vec![ka.clone(), kb.clone()];
    let caps = EngineCaps::default();
    let s = sampler();
    let mut solver = ChildTrialSolver::new(
        config(&dir, oracle.clone()),
        s.clone(),
        Box::new(LocalValuationJudge),
        caps.clone(),
    )
    .unwrap();
    let jm = minimise_joint(
        &keys,
        &[0, 0, 0],
        &list,
        &policy,
        &mut solver,
        &mut StubSystemPoles,
    )
    .unwrap();
    assert_eq!(jm.pieces.len(), 1);
    assert_eq!(jm.pieces[0].keys, keys);
    assert_eq!(
        jm.pieces[0].list,
        vec![vec![0, 0, 0], vec![1, 0, 0], vec![0, 0, -1]]
    );
    // the fake child's coefficients are 1/(d-4): pole order 1 everywhere (the judge ran on the records)
    assert!(
        jm.pieces[0]
            .per_key
            .iter()
            .all(|f| f.pole == PoleOrder::Order { order: 1 })
    );
    // the fake writes no seeds_used: support unavailable at the spread keys
    assert!(
        jm.record["spread"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["support"] == "unavailable")
    );
    let jobs = solver.stats.jobs;
    assert!(jobs > 5, "{jobs} jobs");
    assert!(
        std::fs::read_to_string(dir.join("relations.jsonl"))
            .unwrap()
            .lines()
            .count()
            >= 2
    );
    // run directories removed after settling
    assert_eq!(
        std::fs::read_dir(dir.join("work/runs"))
            .map(|d| d.count())
            .unwrap_or(0),
        0
    );
    // RESUME: a new solver on the same work root answers everything from the journal
    let mut again = ChildTrialSolver::new(
        config(&dir, oracle.clone()),
        s.clone(),
        Box::new(LocalValuationJudge),
        caps,
    )
    .unwrap();
    let jm2 = minimise_joint(
        &keys,
        &[0, 0, 0],
        &list,
        &policy,
        &mut again,
        &mut StubSystemPoles,
    )
    .unwrap();
    assert_eq!(jm2.record, jm.record);
    assert_eq!(again.stats.jobs, 0);
    assert!(again.stats.journal_hits > 0);
    // a journal line with an UNDECIDED answer (an interrupted job) is not replayed: the trial runs again
    let jp = dir.join("work/journal.jsonl");
    let mut text = std::fs::read_to_string(&jp).unwrap();
    let first: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    let mut bad = first.clone();
    let n = bad["answers"].as_array().unwrap().len();
    bad["answers"] = serde_json::json!(vec![
        serde_json::json!({"answer": "undecided", "class": "refused", "message": "killed"});
        n
    ]);
    text = format!(
        "{}\n{}",
        bad,
        text.lines().skip(1).collect::<Vec<_>>().join("\n")
    );
    std::fs::write(&jp, text + "\n").unwrap();
    let mut third = ChildTrialSolver::new(
        config(&dir, oracle.clone()),
        s.clone(),
        Box::new(LocalValuationJudge),
        EngineCaps::default(),
    )
    .unwrap();
    let jm3 = minimise_joint(
        &keys,
        &[0, 0, 0],
        &list,
        &policy,
        &mut third,
        &mut StubSystemPoles,
    )
    .unwrap();
    assert_eq!(jm3.record, jm.record);
    assert_eq!(third.stats.jobs, 1, "only the undecided trial runs again");
    // rows at d0 on an engine without probe mode: typed refusal before any job
    let mut d0 = policy.clone();
    d0.rows_at_d0 = true;
    let e = minimise_joint(
        &keys,
        &[0, 0, 0],
        &list,
        &d0,
        &mut again,
        &mut StubSystemPoles,
    )
    .unwrap_err();
    assert!(
        matches!(e, SeedMinError::EngineLacksProbeMode { .. }),
        "{e}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn engine_capabilities_come_from_the_manifest_without_a_process() {
    let dir = root("manifest");
    let m = dir.join("version.json");
    std::fs::write(
        &m,
        r#"{"program":"rr_node","features":["job","solve_recipe","probe_d"]}"#,
    )
    .unwrap();
    let c = engine_caps_from_manifest(&m);
    assert!(c.probe_d && !c.minimise && !c.discovery_d);
    let none = engine_caps_from_manifest(&dir.join("absent.json"));
    assert!(!none.probe_d && none.features.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Pole orders the in-process judge leaves undecided come from ONE `pole-regular-at` child job per batch (lane
/// coreseed's kind; the fake child's test rule: order = number of rhs keys `I(9,..`), before the answers are journaled.
#[test]
fn undecided_pole_orders_come_from_the_pole_regular_at_child_job() {
    use rustred_campaign::closeloop::coreseed::PolesCfg;
    use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RssPlan};
    use rustred_campaign_core::seedmin::StubRelationPoles;
    assert!(
        rustred_campaign::launch::in_private_namespace(),
        "run under LIGHT_PIDNS=1 (private pid namespace, rule L10)"
    );
    let dir = root("poles");
    let ka = vec![3, 2, -1];
    let kb = vec![2, 3, -1];
    let mut oracle = FakeOracle {
        ms: 5,
        ..Default::default()
    };
    oracle.alts.insert(
        ka.clone(),
        vec![FakeAlt {
            needs: vec![vec![0, 0, 0]],
            rhs: vec![vec![9, 1, -1]],
        }],
    );
    oracle.alts.insert(
        kb.clone(),
        vec![FakeAlt {
            needs: vec![vec![0, 0, 0]],
            rhs: vec![vec![2, 2, -1]],
        }],
    );
    let mut cfg = config(&dir, oracle);
    let mut program =
        Program::from_file("rr_fake_child", Path::new(FAKE), Invocation::Contract).unwrap();
    program.features_required = vec!["fake".into()];
    cfg.poles = Some(PolesCfg {
        program,
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
            faults: vec![],
            time_scale: 1.0,
            volatile_jitter: false,
        }),
    });
    let mut solver = ChildTrialSolver::new(
        cfg,
        sampler(),
        Box::new(StubRelationPoles),
        EngineCaps::default(),
    )
    .unwrap();
    let policy = SeedMinPolicy::default();
    let list = vec![vec![0, 0, 0], vec![1, 0, 0]];
    let jm = minimise_joint(
        &[ka.clone(), kb.clone()],
        &[0, 0, 0],
        &list,
        &policy,
        &mut solver,
        &mut StubSystemPoles,
    )
    .unwrap();
    assert!(solver.stats.pole_jobs >= 1);
    let p = &jm.pieces[0];
    let pole_of = |k: &Vec<i32>| p.per_key[p.keys.iter().position(|x| x == k).unwrap()].pole;
    assert_eq!(pole_of(&ka), PoleOrder::Order { order: 1 });
    assert_eq!(pole_of(&kb), PoleOrder::Order { order: 0 });
    let _ = std::fs::remove_dir_all(&dir);
}
