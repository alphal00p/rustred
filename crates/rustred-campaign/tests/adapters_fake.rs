//! Adapter paths with FAKE children through the REAL launcher (lane adapters; interfaces/adapters.md 4): solve-recipe
//! via `rr_fake_child` (kind solve-recipe emulated), rr_proof via a legacy stub script, generate via `rr_fake_child`
//! (kind fake). Run under `LIGHT_PIDNS=1` (private pid namespace, rule L10):
//! `LIGHT_PIDNS=1 light_build.sh adapters test -p rustred-campaign --test adapters_fake`

mod adapters_support;
use adapters_support::*;

use rustred_campaign::adapters::conformance::Mismatch;
use rustred_campaign::adapters::generate::{GenerateAdapter, GenerateRequest, SeedOptions};
use rustred_campaign::adapters::job::{
    Ended, InputFile, Invocation, JobAdapter, Program, RunOptions, run_attempt,
};
use rustred_campaign::adapters::proof::{FactRequest, ProofInstantiateAdapter, fact_request};
use rustred_campaign::adapters::solve_recipe::{
    BatchPolicy, ClosureRelationsV1, FailureContext, RecipeInput, RelationSink, SolveRecipeAdapter,
    SolveRecipeConfig, SolveRequest,
};
use rustred_campaign_contract::classify::recipe::{FailureClass, KeyRoute, Verdict};
use rustred_campaign_contract::classify::{Accept, Action, WaitStatus};
use rustred_campaign_contract::fake::{
    FakeOp, FakeRule, FakeScript, Fault, FaultKind, OnSigterm, RecipeQuirk, RssPlan,
};
use rustred_campaign_contract::exit::ExitCode;
use rustred_campaign_contract::spec::JobKind;
use rustred_campaign_contract::stop::{CaseResult, StopCode};
use serde_json::json;
use std::path::Path;
use std::time::Duration;

const ORDER: &str = r#"{"name":"o","slot_permutation":null,"coordinate_priority":null,"source_order":"Input"}"#;

fn config(root: &Root, fake: Option<FakeScript>) -> SolveRecipeConfig {
    SolveRecipeConfig {
        program: fake_program(),
        family: root.file("family.sym", b"fake family\n"),
        order: ORDER.into(),
        batch: BatchPolicy {
            max_keys_per_job: 16,
            max_threads: 4,
            min_keys_per_thread: 2,
        },
        max_trace_rows: None,
        key_budget_s: None,
        expect: None,
        determinism_id: None,
        heartbeat_s: 1,
        stop_grace_s: 5,
        package: None,
        live_repair: false,
        licence_env: None,
        fake,
    }
}

fn recipe(root: &Root) -> RecipeInput {
    RecipeInput {
        id: "R/face-1".into(),
        file: root.file("recipe.json", b"{\"fake\":\"recipe\"}\n"),
        digest: None,
    }
}

fn req(r: &RecipeInput, key: &[i64]) -> SolveRequest {
    SolveRequest {
        recipe: r.clone(),
        key: key.to_vec(),
        tag: None,
        context: Some(FailureContext {
            request_key: Some(vec![9; key.len()]),
            rule_chain: vec![json!({"table": "T", "rule": 3, "key": key})],
        }),
    }
}

fn run_solve(
    root: &Root,
    adapter: &SolveRecipeAdapter,
    reqs: &[SolveRequest],
    opts: &RunOptions,
) -> rustred_campaign::adapters::solve_recipe::SolveBatch {
    let plans = adapter.plan(reqs).unwrap();
    assert_eq!(plans.len(), 1);
    let mut l = launcher();
    let (out, rep) = run_attempt(&mut l, adapter, reqs, &plans[0], root.path(), 1, opts).unwrap();
    assert!(rep.launch_error.is_none(), "{:?}", rep.launch_error);
    eprintln!(
        "wait {:?} -> row {}{} {:?} ({})",
        rep.ended.wait,
        out.classification.row,
        out.classification.sub,
        out.classification.kind,
        out.classification.why
    );
    out
}

// ------------------------------------------------------------------------------------------------ pure (no process)

#[test]
fn plan_batches_by_recipe_with_threads_from_data_and_is_deterministic() {
    let root = Root::new("plan");
    let a = recipe(&root);
    let mut b = recipe(&root);
    b.file = root.file("recipe-b.json", b"{\"b\":1}\n");
    let mut c = config(&root, None);
    c.batch = BatchPolicy {
        max_keys_per_job: 2,
        max_threads: 8,
        min_keys_per_thread: 1,
    };
    let ad = SolveRecipeAdapter::new(c);
    let reqs = vec![
        req(&a, &[1, 1, 2]),
        req(&b, &[1, 2, 1]),
        req(&a, &[1, 1, 3]),
        req(&a, &[1, 1, 2]), // duplicate: solved once, both requests served
        req(&a, &[1, 1, 4]),
        req(&b, &[2, 1, 1]),
    ];
    let p1 = ad.plan(&reqs).unwrap();
    let p2 = ad.plan(&reqs).unwrap();
    assert_eq!(p1, p2, "plans are deterministic");
    // recipe a: unique keys [112, 113, 114] -> 2 jobs; recipe b: 2 keys -> 1 job
    assert_eq!(p1.len(), 3);
    assert_eq!(p1[0].members, vec![0, 3, 2]);
    assert_eq!(p1[1].members, vec![4]);
    assert_eq!(p1[2].members, vec![1, 5]);
    assert!(p1.iter().all(|p| p.job_id.ends_with(".sr")));
    let bp = BatchPolicy {
        max_keys_per_job: 100,
        max_threads: 4,
        min_keys_per_thread: 3,
    };
    assert_eq!(bp.threads_for(1), 1);
    assert_eq!(bp.threads_for(7), 3);
    assert_eq!(bp.threads_for(100), 4);
    // keys of one recipe must share a length (generic in arity, but consistent)
    let bad = vec![req(&a, &[1, 1]), req(&a, &[1, 1, 1])];
    assert!(ad.plan(&bad).is_err());
}

#[test]
fn solve_recipe_spec_for_a_real_engine_validates_with_the_contract() {
    let root = Root::new("spec");
    let mut c = config(&root, None);
    c.program = Program {
        name: "rr_node".into(),
        path: Path::new(FAKE).to_path_buf(),
        blake3: c.program.blake3.clone(),
        invocation: Invocation::VerbJobFlag {
            verb: "solve-recipe".into(),
        },
        features_required: vec![],
    };
    c.max_trace_rows = Some(5000);
    c.key_budget_s = Some(60);
    let ad = SolveRecipeAdapter::new(c);
    let r = recipe(&root);
    let reqs = vec![req(&r, &[1, 2, 3]), req(&r, &[1, 2, 4])];
    let plan = &ad.plan(&reqs).unwrap()[0];
    let p = ad.prepare(&reqs, plan, root.path(), 1).unwrap();
    assert_eq!(p.spec.kind, JobKind::SolveRecipe);
    assert_eq!(
        p.spec.args,
        vec![
            "--order", ORDER, "--threads", "1", "--max-trace-rows", "5000", "--key-budget-s", "60"
        ]
    );
    let roles: Vec<&str> = p.spec.inputs.iter().map(|i| i.role.as_str()).collect();
    assert_eq!(roles, vec!["family", "recipe", "keys"]);
    // the launch line is the verb's job form, the keys input is RUN-relative
    let argv: Vec<String> = p.launch.args.iter().map(|a| a.to_string_lossy().into()).collect();
    assert_eq!(argv[0], "solve-recipe");
    assert_eq!(argv[1], "--job");
    assert_eq!(p.spec.inputs[2].path, "in/keys.jsonl");
    let keys = std::fs::read_to_string(p.layout.in_dir().join("keys.jsonl")).unwrap();
    assert_eq!(keys, "{\"key\":[1,2,3]}\n{\"key\":[1,2,4]}\n");
    // an attempt directory is never reused
    assert!(ad.prepare(&reqs, plan, root.path(), 1).is_err());
}

#[test]
fn generate_seed_options_map_one_to_one_and_validate() {
    let root = Root::new("gen-spec");
    let fam = root.file("family.sym", b"f\n");
    let mut prog = fake_program();
    prog.name = "rr_node".into();
    let ad = GenerateAdapter {
        program: prog,
        licence_env: None,
        heartbeat_s: 1,
        stop_grace_s: 5,
        fake: None,
    };
    let s = SeedOptions::symmetry_at_key_alone();
    assert_eq!(
        s.args(),
        vec![
            "--max-depth",
            "0",
            "--numeric-depth",
            "0",
            "--symmetry-symbolic",
            "--open-on-refusal"
        ]
    );
    let mut seed = SeedOptions {
        max_depth: Some(2),
        centres: Some("6:-1;9:-1".into()),
        face_floor: Some(1),
        layer_cap: Some("0,0,1".into()),
        screen: Some("40,8,24,2000".into()),
        no_screen: false,
        ..SeedOptions::default()
    };
    seed.symmetry_symbolic = true;
    let reqs = vec![
        GenerateRequest {
            family: fam.clone(),
            cases: vec![json!({"frame":"presented","fixed":[1,null,0],"seed_centre_offset":[0,-1,0]})],
            order: "id".into(),
            seed,
            max_cases: Some(1),
            descend_open: true,
            wall_case_s: Some(300),
            wall_job_s: None,
        },
        GenerateRequest {
            family: fam,
            cases: vec![],
            order: "id".into(),
            seed: SeedOptions::symmetry_at_key_alone(),
            max_cases: Some(1),
            descend_open: false,
            wall_case_s: None,
            wall_job_s: None,
        },
    ];
    let plans = ad.plan(&reqs).unwrap();
    assert_eq!(plans.len(), 2);
    for p in &plans {
        let prep = ad.prepare(&reqs, p, root.path(), 1).unwrap();
        assert_eq!(prep.spec.kind, JobKind::Generate);
        prep.spec.validate().unwrap();
    }
}

// ------------------------------------------------------------------------------------------------ solve-recipe

#[test]
fn all_solved_is_row_1_and_relations_reach_the_relation_file() {
    require_namespace();
    let _g = serial();
    let root = Root::new("solved");
    let r = recipe(&root);
    let reqs = vec![req(&r, &[3, 1, 1]), req(&r, &[4, 1, 1]), req(&r, &[3, 1, 1])];
    let ad = SolveRecipeAdapter::new(config(
        &root,
        Some(recipe_script(&[Verdict::Solved, Verdict::Solved], &[3, 1, 1], &[])),
    ));
    let out = run_solve(&root, &ad, &reqs, &RunOptions::default());
    assert_eq!(out.classification.row, 1, "{}", out.classification.why);
    assert_eq!(out.classification.accept, Accept::Canonical);
    assert!(out.refusal.is_none() && out.failures.is_empty() && out.not_decided.is_empty());
    assert_eq!(out.relations.len(), 2);
    assert_eq!(out.relations[0].requests, vec![0, 2]);
    assert_eq!(out.relations[1].requests, vec![1]);
    assert_eq!(out.relations[0].rhs_keys, vec![vec![2, 1, 1]]);
    assert_eq!(out.relations[0].verdict_bits, 7);
    // the relation file (closure.md 12) reads back with the same keys and digests
    let mut sink = ClosureRelationsV1 {
        dir: root.path().join("rel"),
        bits: 8,
    };
    let files = sink.write(&out.relations).unwrap();
    assert_eq!(files.len(), 1);
    let f = std::fs::File::open(&files[0].0).unwrap();
    let (_name, rd, store) =
        rustred_campaign_core::closure::RelationStore::read(std::io::BufReader::new(f)).unwrap();
    assert_eq!(rd, out.relations[0].recipe_digest);
    assert_eq!(store.len(), 2);
    let i = store.find(&[3, 1, 1]).unwrap();
    assert_eq!(store.key(i), vec![3, 1, 1]);
    assert_eq!(store.verdict(i), 7);
}

#[test]
fn unsolved_keys_are_row_28_with_typed_r48_failures() {
    require_namespace();
    let _g = serial();
    let root = Root::new("row28");
    let r = recipe(&root);
    let reqs: Vec<SolveRequest> = (1..=4).map(|i| req(&r, &[i, 1, 1])).collect();
    let ad = SolveRecipeAdapter::new(config(
        &root,
        Some(recipe_script(
            &[
                Verdict::Solved,
                Verdict::NoRelationInSeeds,
                Verdict::NotInDomain,
                Verdict::Irregular,
            ],
            &[1, 1, 1],
            &[],
        )),
    ));
    let out = run_solve(&root, &ad, &reqs, &RunOptions::default());
    assert_eq!(out.classification.row, 28, "{}", out.classification.why);
    assert_eq!(out.classification.accept, Accept::SolvedRecords);
    assert_eq!(out.classification.action, Action::QueueFailures);
    assert_eq!(out.relations.len(), 1);
    assert_eq!(out.failures.len(), 3);
    let routes: Vec<(FailureClass, KeyRoute)> =
        out.failures.iter().map(|f| (f.class, f.route)).collect();
    assert_eq!(
        routes,
        vec![
            (FailureClass::NoRelationInSeeds, KeyRoute::NextRecipeRung),
            (FailureClass::NotInDomain, KeyRoute::NextRecipeRung),
            (FailureClass::Irregular, KeyRoute::SubCase),
        ]
    );
    let f = &out.failures[0];
    // R48 addendum 3b: identity, key, original key and chain, class, determinism id, binary, ONE-LINE reproduction
    assert_eq!(f.schema, "rustred.campaign.recipe-failure.v1");
    assert_eq!(f.key, vec![2, 1, 1]);
    assert_eq!(f.request_key, Some(vec![9, 9, 9]));
    assert_eq!(f.rule_chain.len(), 1);
    assert_eq!(f.recipe.id, "R/face-1");
    assert_eq!(f.recipe.digest.len(), 64);
    assert!(!f.determinism_id.is_empty());
    assert_eq!(f.binary.blake3, ad.config.program.blake3);
    assert!(!f.reproduce.contains('\n'));
    assert!(f.reproduce.contains(" solve-recipe --family '"));
    assert!(f.reproduce.contains(" --key 2,1,1 --out '"));
    assert!(f.child_reproduce.is_some());
    assert!(serde_json::from_str::<serde_json::Value>(&f.to_json_line()).is_ok());
}

#[test]
fn no_solved_key_is_row_29_and_budget_work_goes_to_the_next_work_class() {
    require_namespace();
    let _g = serial();
    let root = Root::new("row29");
    let r = recipe(&root);
    let reqs: Vec<SolveRequest> = (1..=2).map(|i| req(&r, &[i, 0])).collect();
    let mut c = config(
        &root,
        Some(recipe_script(&[Verdict::BudgetWork, Verdict::BudgetWork], &[1, 0], &[])),
    );
    c.max_trace_rows = Some(10);
    let ad = SolveRecipeAdapter::new(c);
    let out = run_solve(&root, &ad, &reqs, &RunOptions::default());
    assert_eq!(out.classification.row, 29, "{}", out.classification.why);
    assert_eq!(out.classification.accept, Accept::Nothing);
    assert!(out.relations.is_empty());
    assert_eq!(out.failures.len(), 2);
    assert!(out.failures.iter().all(|f| f.route == KeyRoute::NextWorkClass));
    assert!(out.failures[0].reproduce.contains("--max-trace-rows 10"));
}

#[test]
fn a_per_key_wall_budget_is_time_decided() {
    require_namespace();
    let _g = serial();
    let root = Root::new("wall");
    let r = recipe(&root);
    let reqs: Vec<SolveRequest> = (1..=2).map(|i| req(&r, &[i, 0])).collect();
    let mut c = config(
        &root,
        Some(recipe_script(&[Verdict::Solved, Verdict::BudgetWall], &[1, 0], &[])),
    );
    c.key_budget_s = Some(30);
    let ad = SolveRecipeAdapter::new(c);
    let out = run_solve(&root, &ad, &reqs, &RunOptions::default());
    assert_eq!(out.classification.row, 28, "{}", out.classification.why);
    assert!(out.classification.time_decided);
    assert_eq!(out.failures[0].route, KeyRoute::LadderDecides);
}

fn slow_script(n: usize) -> FakeScript {
    let mut s = recipe_script(&vec![Verdict::Solved; n], &[5, 0], &[]);
    for op in s.script.iter_mut() {
        if let FakeOp::Relation { ms, .. } = op {
            *ms = 1500;
        }
    }
    s
}

#[test]
fn a_stop_file_interrupts_row_11_and_every_key_is_left_undecided() {
    require_namespace();
    let _g = serial();
    let root = Root::new("stop");
    let r = recipe(&root);
    let reqs: Vec<SolveRequest> = (5..=7).map(|i| req(&r, &[i, 0])).collect();
    let ad = SolveRecipeAdapter::new(config(&root, Some(slow_script(3))));
    let plan = ad.plan(&reqs).unwrap().remove(0);
    let run = rustred_campaign_contract::layout::attempt_dir(
        &std::path::absolute(root.path()).unwrap(),
        &plan.job_id,
        1,
    );
    let t = std::thread::spawn(move || {
        for _ in 0..200 {
            if run.join("EVENTS.jsonl").exists() {
                std::thread::sleep(Duration::from_millis(300));
                std::fs::write(run.join("STOP"), b"stop\n").unwrap();
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    let mut l = launcher();
    let (out, _) = run_attempt(&mut l, &ad, &reqs, &plan, root.path(), 1, &RunOptions::default())
        .unwrap();
    t.join().unwrap();
    assert_eq!(out.classification.row, 11, "{}", out.classification.why);
    assert_eq!(out.classification.sub, "");
    assert!(out.relations.is_empty(), "nothing from an interrupted attempt (R-b)");
    assert_eq!(out.not_decided.len(), 3);
    assert!(out.refusal.is_none());
}

#[test]
fn the_supervisor_wall_cap_is_row_11b_time_decided() {
    require_namespace();
    let _g = serial();
    let root = Root::new("cap");
    let r = recipe(&root);
    let reqs: Vec<SolveRequest> = (5..=7).map(|i| req(&r, &[i, 0])).collect();
    let ad = SolveRecipeAdapter::new(config(&root, Some(slow_script(3))));
    let opts = RunOptions {
        wall_cap: Some(Duration::from_millis(700)),
        grace: Some(Duration::from_secs(10)),
        ..RunOptions::default()
    };
    let out = run_solve(&root, &ad, &reqs, &opts);
    assert_eq!((out.classification.row, out.classification.sub.as_str()), (11, "b"));
    assert!(out.classification.time_decided);
    assert!(out.relations.is_empty());
}

fn quirk_refusal(q: RecipeQuirk, verdicts: &[Verdict]) -> rustred_campaign::adapters::solve_recipe::SolveBatch {
    let root = Root::new("quirk");
    let r = recipe(&root);
    let reqs: Vec<SolveRequest> = (1..=verdicts.len() as i64).map(|i| req(&r, &[i, 0])).collect();
    let mut c = config(&root, Some(recipe_script(verdicts, &[1, 0], &[q])));
    c.key_budget_s = Some(5);
    let ad = SolveRecipeAdapter::new(c);
    run_solve(&root, &ad, &reqs, &RunOptions::default())
}

#[test]
fn deviations_of_the_child_are_refused_and_diagnosed() {
    require_namespace();
    let _g = serial();
    let two = [Verdict::Solved, Verdict::Solved];
    // the engine line's verb: OUTCOME spec digest = plain BLAKE3 of job.json
    let o = quirk_refusal(RecipeQuirk::PlainSpecDigest, &two);
    assert_eq!(o.classification.row, 22);
    assert!(o.relations.is_empty());
    let r = o.refusal.expect("refusal");
    assert!(r.conformance.contains(&Mismatch::SpecDigestPlainBlake3), "{r:?}");
    // outputs.extra without canonical_blake3
    let o = quirk_refusal(RecipeQuirk::NoCanonicalDigest, &two);
    assert_eq!(o.classification.row, 22);
    let r = o.refusal.expect("refusal");
    assert!(r.conformance.contains(&Mismatch::ExtraWithoutCanonicalDigest {
        role: "results".into()
    }));
    // effective without its digest: OUTCOME unreadable by the contract
    let o = quirk_refusal(RecipeQuirk::EffectiveWithoutDigest, &two);
    assert_eq!(o.classification.row, 22);
    let r = o.refusal.expect("refusal");
    assert!(r.conformance.contains(&Mismatch::EffectiveWithoutDigest));
    assert!(r
        .conformance
        .iter()
        .any(|m| matches!(m, Mismatch::OutcomeUnparsable { .. })));
    // record-level deviations make S false: nothing accepted
    for (q, why) in [
        (RecipeQuirk::UnprovedSolved, "exact proof"),
        (RecipeQuirk::WrongKey, "input line's key"),
    ] {
        let o = quirk_refusal(q, &two);
        assert_eq!(o.classification.row, 22, "{q:?}");
        assert!(o.relations.is_empty());
        let r = o.refusal.expect("refusal");
        assert!(r.unsealed_why.as_deref().unwrap_or("").contains(why), "{r:?}");
    }
    let o = quirk_refusal(RecipeQuirk::NoTimeDecision, &[Verdict::Solved, Verdict::BudgetWall]);
    assert_eq!(o.classification.row, 22);
    assert!(o.refusal.unwrap().unsealed_why.unwrap().contains("time_decided"));
}

#[test]
fn a_record_of_another_recipe_digest_is_refused_against_its_request() {
    require_namespace();
    let _g = serial();
    let root = Root::new("digest");
    let mut r = recipe(&root);
    r.digest = Some("0".repeat(64));
    let reqs = vec![req(&r, &[1, 0])];
    let ad = SolveRecipeAdapter::new(config(
        &root,
        Some(recipe_script(&[Verdict::Solved], &[1, 0], &[])),
    ));
    let out = run_solve(&root, &ad, &reqs, &RunOptions::default());
    assert_eq!(out.classification.row, 1);
    assert!(out.relations.is_empty());
    assert_eq!(out.refused_records.len(), 1);
    assert!(out.refused_records[0].why.contains("recipe digest"));
}

#[test]
fn errors_and_crashes_map_to_their_rows() {
    require_namespace();
    let _g = serial();
    for (case, row) in [("input", 13u8), ("internal", 17), ("abort", 23), ("exit", 23)] {
        let root = Root::new(case);
        let r = recipe(&root);
        let reqs = vec![req(&r, &[1, 0])];
        let mut s = recipe_script(&[Verdict::Solved], &[1, 0], &[]);
        match case {
            "input" => s.script.insert(
                1,
                FakeOp::Fail {
                    code: ExitCode::Input,
                    message: "bad keys".into(),
                },
            ),
            "internal" => s.script.insert(
                1,
                FakeOp::Fail {
                    code: ExitCode::Internal,
                    message: "boom".into(),
                },
            ),
            "abort" => s.faults.push(Fault {
                at_op: 1,
                kind: FaultKind::Abort,
                bytes: 0,
                code: 0,
                path: None,
            }),
            _ => s.faults.push(Fault {
                at_op: 1,
                kind: FaultKind::Exit,
                bytes: 0,
                code: 3,
                path: None,
            }),
        }
        let ad = SolveRecipeAdapter::new(config(&root, Some(s)));
        let out = run_solve(&root, &ad, &reqs, &RunOptions::default());
        assert_eq!(out.classification.row, row, "{case}: {}", out.classification.why);
        assert!(out.relations.is_empty());
        assert_eq!(out.not_decided.len(), 1);
        assert!(out.refusal.is_some(), "{case}");
    }
}

#[test]
fn unknown_wait_consumes_the_sealed_results_of_an_unconfirmed_launch() {
    require_namespace();
    let _g = serial();
    let root = Root::new("unknown");
    let r = recipe(&root);
    let reqs = vec![req(&r, &[1, 0]), req(&r, &[2, 0])];
    let ad = SolveRecipeAdapter::new(config(
        &root,
        Some(recipe_script(&[Verdict::Solved, Verdict::NotInDomain], &[1, 0], &[])),
    ));
    let plan = ad.plan(&reqs).unwrap().remove(0);
    let mut l = launcher();
    let (first, rep) =
        run_attempt(&mut l, &ad, &reqs, &plan, root.path(), 1, &RunOptions::default()).unwrap();
    // recovery hazard rule: the launch was never confirmed -> execution UNKNOWN, classified from the files (6.6)
    let again = ad.finish(&reqs, &plan, &rep.prepared, &Ended::unknown());
    assert_eq!(again.classification.row, 28);
    assert_eq!(again.relations, first.relations);
    assert_eq!(again.failures.len(), first.failures.len());
}

// ------------------------------------------------------------------------------------------------ rr_proof (legacy)

fn table(root: &Root) -> (InputFile, u64, u64) {
    let header = "{\"type\":\"header\",\"schema\":\"t\"}\n";
    let r0 = "{\"type\":\"rule\",\"index\":0,\"target\":\"I(a)\"}\n";
    let r1 = "{\"type\":\"rule\",\"index\":1,\"target\":\"I(b)\"}\n";
    let f = root.file("T.table.jsonl", format!("{header}{r0}{r1}").as_bytes());
    (f, header.len() as u64, (header.len() + r0.len()) as u64)
}

fn answer(r: &FactRequest, key: &[i64], with_sha: bool) -> String {
    let mut v = json!({"schema": "rustred.proof-kernel.instantiate.v1", "rule": r.rule, "key": key,
        "in_case": true, "chart_ok": true, "branch": null, "regular": "regular", "irregular_terms": [],
        "hidden_loci": [], "rhs": [], "zero_test": "exact", "error": null});
    if with_sha {
        v["record_sha256"] = json!(r.record_sha256);
    }
    v.to_string()
}

fn stub(root: &Root, body: &str) -> Program {
    let p = root.path().join("rr_proof_stub.sh");
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    Program::from_file("rr_proof", &p, Invocation::Legacy).unwrap()
}

#[test]
fn rr_proof_legacy_child_is_sealed_by_the_wrapper_and_bound_by_record_sha256() {
    require_namespace();
    let _g = serial();
    for mode in ["ok", "exit2", "nobinding", "missing"] {
        let root = Root::new(&format!("proof-{mode}"));
        let (t, o0, o1) = table(&root);
        let reqs = vec![
            fact_request(&t, o0, 0, vec![vec![1, 1], vec![2, 1]]).unwrap(),
            fact_request(&t, o1, 1, vec![vec![1, 2]]).unwrap(),
        ];
        assert_eq!(reqs[0].record_sha256.len(), 64);
        let mut lines = vec![
            answer(&reqs[0], &[1, 1], mode != "nobinding"),
            answer(&reqs[0], &[2, 1], true),
            answer(&reqs[1], &[1, 2], true),
        ];
        if mode == "missing" {
            lines.pop();
        }
        let ans = root.path().join("answers.jsonl");
        std::fs::write(&ans, lines.join("\n") + "\n").unwrap();
        let body = match mode {
            "exit2" => "echo '{\"verdict\":\"ERROR\",\"error\":\"bad requests\"}'; exit 2".to_string(),
            _ => format!("cp '{}' \"$5\"", ans.display()),
        };
        let ad = ProofInstantiateAdapter {
            program: stub(&root, &body),
            licence_env: None,
        };
        let plans = ad.plan(&reqs).unwrap();
        assert_eq!(plans.len(), 1, "one job per table");
        let mut l = launcher();
        let (out, _) =
            run_attempt(&mut l, &ad, &reqs, &plans[0], root.path(), 1, &RunOptions::default())
                .unwrap();
        eprintln!("{mode}: row {} ({})", out.classification.row, out.classification.why);
        match mode {
            "ok" => {
                assert_eq!(out.classification.row, 1);
                assert_eq!(out.facts.len(), 3);
                assert_eq!(out.facts[2].requests, vec![1]);
                assert!(out.refusal.is_none());
            }
            "exit2" => {
                assert_eq!(out.legacy_wait, WaitStatus::Exited(2));
                assert_eq!(out.mapped_wait, WaitStatus::Exited(41));
                assert_eq!(out.classification.row, 13);
                assert!(out.facts.is_empty());
            }
            "nobinding" => {
                assert_eq!(out.classification.row, 22);
                let r = out.refusal.unwrap();
                assert!(r.conformance.contains(&Mismatch::AnswerWithoutBinding {
                    field: "record_sha256".into()
                }));
            }
            _ => {
                assert_eq!(out.classification.row, 22);
                assert!(out.facts.is_empty());
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ generate

#[test]
fn generate_through_the_fake_child_accepts_the_verified_table() {
    require_namespace();
    let _g = serial();
    let root = Root::new("gen");
    let fam = root.file("family.sym", b"f\n");
    let script = FakeScript {
        script: vec![
            FakeOp::Prepare { ms: 10 },
            FakeOp::Case {
                ordinal: 0,
                ms: 50,
                work: Default::default(),
                result: CaseResult::Rule,
                rule: Some(FakeRule {
                    record: json!({"target": "I(1,1)", "rhs": [["I(1,0)", "1"]]}),
                    loci: None,
                }),
                children: vec![],
            },
            FakeOp::End {
                stop: Some(StopCode::MaxCases),
                verdict: None,
            },
        ],
        on_sigterm: OnSigterm::Graceful,
        slow_ms: 0,
        start_delay_ms: 0,
        rss: RssPlan::default(),
        helpers: vec![],
        faults: vec![],
        time_scale: 1.0,
        volatile_jitter: false,
    };
    let ad = GenerateAdapter {
        program: fake_program(),
        licence_env: None,
        heartbeat_s: 1,
        stop_grace_s: 5,
        fake: Some(script),
    };
    let reqs = vec![GenerateRequest {
        family: fam,
        cases: vec![],
        order: "id".into(),
        seed: SeedOptions::default(),
        max_cases: Some(1),
        descend_open: false,
        wall_case_s: None,
        wall_job_s: None,
    }];
    let plans = ad.plan(&reqs).unwrap();
    let mut l = launcher();
    let (out, _) =
        run_attempt(&mut l, &ad, &reqs, &plans[0], root.path(), 1, &RunOptions::default()).unwrap();
    assert_eq!(out.classification.row, 2, "{}", out.classification.why);
    assert!(out.refusal.is_none());
    let rules = out
        .accepted
        .iter()
        .filter(|l| String::from_utf8_lossy(l).contains("\"type\":\"rule\""))
        .count();
    assert_eq!(rules, 1);
}
