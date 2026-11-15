//! Lane closeloop (interfaces/closeloop.md 1-8): the close-from-material work source under a REAL `rr_campaign`
//! supervisor with `rr_fake_child` emulating kind solve-recipe. Spawns processes and sends signals: run ONLY inside a
//! private pid namespace (`LIGHT_PIDNS=1 .../light_build.sh closeloop test -p rustred-campaign --test closeloop_fake`).
//!
//! The node is a TOY (arity 3, chart 110; closeloop_support): a chain of keys that needs several rounds of recipe solves,
//! a second face whose answers reach into the first, and a key first met while closing that only the default recipe's
//! domain contains (R49).

mod closeloop_support;
mod sup_common;

use closeloop_support::*;
use rustred_campaign::closeloop::pass::{PassIn, run_pass};
use rustred_campaign::ledger::{Event, ItemEvent};
use rustred_campaign::state::{ItemPhase, NodeEvent, NodePhase};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;
use sup_common::*;

fn setup(tag: &str, toy_cfg: &Toy) -> (Tmp, ToyFiles, std::path::PathBuf, std::path::PathBuf) {
    let t = Tmp::new(tag);
    let files = toy(&t.path.join("material"), toy_cfg, FAKE);
    let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, files, sp, cdir)
}

fn result_json(cdir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(cdir.join("closeloop/toy/final/result.json")).unwrap())
        .unwrap()
}

/// Items (id, label, weight, phase word) and the certified relations per accepted solve item.
type Decisions = (
    BTreeSet<(String, String, u64, String)>,
    BTreeMap<String, Vec<Value>>,
);

fn decisions(cdir: &Path) -> Decisions {
    let st = state(cdir);
    let items = st
        .items
        .iter()
        .map(|(k, it)| {
            (
                k.clone(),
                it.label.clone(),
                it.weight,
                it.phase.tag().to_string(),
            )
        })
        .collect();
    let mut cert = BTreeMap::new();
    for (k, it) in st.items.iter() {
        if let Some(a) = &it.accepted
            && it.kind == "recipe-solve"
        {
            let b = std::fs::read(cdir.join(&a.record.path)).unwrap();
            let lines: Vec<Value> = String::from_utf8_lossy(&b)
                .lines()
                .map(|l| serde_json::from_str::<Value>(l).unwrap())
                .filter(|v| v["verdict"] == "SOLVED")
                .map(|v| serde_json::json!([v["source"], v["key"], v["relation_digest"]]))
                .collect();
            cert.insert(k.clone(), lines);
        }
    }
    (items, cert)
}

/// lane closeloop4 (closeloop.md 19): run to the end; an exit 8 for terminal proposals (keys nothing could serve, e.g.
/// after refused answers) is answered by the operator REJECTING them, so the node ends open as before.
fn run_rejecting_proposals(sp: &Path, cdir: &Path) -> Vec<i32> {
    let mut codes = vec![];
    for _ in 0..4 {
        let mut c = start(sp, cdir, &["--plain"], &[]);
        let code = wait(&mut c, Duration::from_secs(900));
        codes.push(code);
        if code != ATTENTION {
            return codes;
        }
        assert!(
            latest_proposals(cdir, "toy").is_some(),
            "exit 8 without proposals: {}",
            log_tail(cdir)
        );
        assert_eq!(resolve_terminals(cdir, Some("toy"), None), 0);
    }
    codes
}

fn node_events(cdir: &Path) -> Vec<String> {
    records(cdir)
        .iter()
        .filter_map(|r| match &r.event {
            Event::Node { ev, .. } => Some(ev.tag().to_string()),
            _ => None,
        })
        .collect()
}

fn accepts(cdir: &Path) -> usize {
    count(cdir, |e| {
        matches!(
            e,
            Event::Item {
                ev: ItemEvent::Accept { .. },
                ..
            }
        )
    })
}

fn answers_lines(cdir: &Path) -> Vec<Value> {
    let mut v = vec![];
    let d = cdir.join("closeloop/toy/answers");
    let Ok(rd) = std::fs::read_dir(&d) else {
        return v;
    };
    let mut paths: Vec<_> = rd.map(|e| e.unwrap().path()).collect();
    paths.sort();
    for p in paths {
        if p.to_string_lossy().ends_with(".jsonl") {
            for l in std::fs::read_to_string(&p).unwrap().lines() {
                v.push(serde_json::from_str(l).unwrap());
            }
        }
    }
    v
}

#[test]
fn pass_round_one_lists_the_first_batches_in_process() {
    // a pure pass (no process): the import + check of round 1 and its batches
    let t = Tmp::new("clpass");
    let files = toy(&t.path.join("m"), &Toy::default(), FAKE);
    let src = rustred_campaign::closeloop::CloseFromMaterialSource::new(
        rustred_campaign::closeloop::CloseLoopConfig::read(&files.config).unwrap(),
        "cfg".into(),
        &t.path.join("cdir"),
        [("rr_fake_child".to_string(), std::path::PathBuf::from(FAKE))]
            .into_iter()
            .collect(),
    );
    assert!(src.is_ok(), "{:?}", src.err());
    let spec_text = std::fs::read_to_string(&files.import_spec).unwrap();
    let mk = |budgets: rustred_campaign::closeloop::config::Budgets, keys_issued: u64| PassIn {
        node: "toy".into(),
        round: 1,
        spec_text: spec_text.clone(),
        base: files.dir.clone(),
        answers: vec![],
        threads: 2,
        required: None,
        issued: BTreeSet::new(),
        keys_issued,
        jobs_issued: 0,
        budgets,
        max_keys_per_job: 2,
        hand: None,
        final_dir: t.path.join("final"),
        attention_items: 0,
        relation_rule: "first_admissible".into(),
        ..PassIn::first("toy", String::new(), Default::default(), Default::default())
    };
    // a key budget of 1: the batch list is cut (not final); with that key already spent: a budget stop (final)
    let small = rustred_campaign::closeloop::config::Budgets {
        max_solve_keys: 1,
        ..Default::default()
    };
    let cut = run_pass(&mk(small, 0), None).unwrap();
    assert_eq!(cut.batches.len(), 1);
    assert!(cut.final_.is_none());
    assert!(
        cut.check["budget"].as_str().unwrap().contains("1 of 2"),
        "{}",
        cut.check
    );
    let stop = run_pass(&mk(small, 1), Some(cut.set.clone())).unwrap();
    assert!(stop.batches.is_empty());
    assert_eq!(
        stop.final_.as_ref().map(|f| f.verdict.as_str()),
        Some("budget")
    );
    let out = run_pass(
        &PassIn {
            node: "toy".into(),
            round: 1,
            spec_text,
            base: files.dir.clone(),
            answers: vec![],
            threads: 2,
            required: None,
            issued: BTreeSet::new(),
            keys_issued: 0,
            jobs_issued: 0,
            budgets: Default::default(),
            max_keys_per_job: 2,
            hand: None,
            final_dir: t.path.join("final"),
            attention_items: 0,
            relation_rule: "first_admissible".into(),
            ..PassIn::first("toy", String::new(), Default::default(), Default::default())
        },
        None,
    )
    .unwrap();
    let b: Vec<(String, Vec<Vec<i32>>)> = out
        .batches
        .iter()
        .map(|b| (b.source.clone(), b.keys.clone()))
        .collect();
    assert_eq!(
        b,
        vec![
            ("R1".to_string(), vec![vec![4, 1, 0]]),
            ("R2".to_string(), vec![vec![3, 2, 0]])
        ],
        "{}",
        out.check
    );
    assert!(out.defect > 0 && out.final_.is_none());
    assert_eq!(out.check["tier"], "C", "{}", out.check);
}

#[test]
fn fake_children_close_the_toy_end_to_end_with_the_default_recipe() {
    if std::env::var_os("CL_DEBUG").is_some() {
        unsafe { std::env::set_var("RR_SUP_KEEP", "1") };
    }
    let _g = serial();
    let (_t, files, sp, cdir) = setup("clclose", &Toy::default());
    // the hand statement (comparison only): the expected closure keys
    let hand: Vec<Value> = [
        ([5, 1, 0], true, "base", "A#0"),
        ([4, 1, 0], false, "recipe", "R1"),
        ([3, 1, 0], false, "recipe", "R1"),
        ([2, 1, 0], false, "recipe", "R1"),
        ([1, 1, 0], false, "recipe", "default-recipe"),
        ([3, 2, 0], true, "recipe", "R2"),
        ([2, 2, 0], false, "recipe", "R2"),
        ([1, 2, 0], false, "recipe", "R2"),
    ]
    .iter()
    .map(|(k, b, l, r)| serde_json::json!([format!("I({},{},{})", k[0], k[1], k[2]), b, l, r]))
    .collect();
    std::fs::write(
        &files.hand,
        hand.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    let st = state(&cdir);
    assert_eq!(
        st.nodes["toy"].state.phase,
        NodePhase::Closed,
        "{:?}\n{}\n{:#}",
        node_events(&cdir),
        log_tail(&cdir),
        result_json(&cdir)["check"]
    );
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "closed", "{r:#}");
    assert_eq!(r["comparison"]["key_sets_equal"], true, "{r:#}");
    assert_eq!(r["check"]["strict"], true);
    // lane closeloop2: the import's per-class counts (`at_required`) and the class x level table
    assert!(
        r["evidence_by_class"]["recipe"]["at_required"]
            .as_u64()
            .unwrap()
            >= 7,
        "{r:#}"
    );
    assert!(
        r["by_rule_class_level"]["recipe"]["per-key"]
            .as_u64()
            .unwrap()
            >= 7,
        "{r:#}"
    );
    // R49: [1,1,0] is in no per-face recipe's domain (R1 covers n0 >= 2): the default recipe certified it
    let ans = answers_lines(&cdir);
    assert!(
        ans.iter().any(|v| v["source"] == "default-recipe"
            && v["key"] == serde_json::json!([1, 1, 0])
            && v["verdict"] == "SOLVED"),
        "{ans:#?}"
    );
    assert!(ans.iter().all(|v| v["relation_rule"].as_str().is_some()));
    // node machine: boxed from the start (lane closeloop3, closeloop.md 16.5: registered `start_target: boxed`, no
    // AttemptEnded + FallBackToBox pair any more), then closed BOXED
    let ev = node_events(&cdir);
    assert!(
        !ev.contains(&"fall_back_to_box".to_string())
            && !ev.contains(&"attempt_ended".to_string())
            && ev.last().map(String::as_str) == Some("declare_closed"),
        "{ev:?}"
    );
    assert_eq!(state(&cdir).nodes["toy"].state.closure_kind(), "boxed");
    // statement + comparison files, progress file
    assert!(
        cdir.join("closeloop/toy/final/statement/statement.jsonl")
            .is_file()
    );
    assert!(
        cdir.join("closeloop/toy/final/comparison/comparison.json")
            .is_file()
    );
    let p: Value =
        serde_json::from_slice(&std::fs::read(cdir.join("progress/closeloop.json")).unwrap())
            .unwrap();
    assert_eq!(p["nodes"][0]["phase"], "closed", "{p:#}");
    assert_eq!(verify(&cdir), 0, "progress verify");
}

#[test]
fn interrupt_and_resume_at_random_points_gives_the_same_decisions() {
    let _g = serial();
    let cfg = Toy {
        top: 6,
        second_face: Some(4),
        ms: 150,
        ..Toy::default()
    };
    // reference run (no interrupt)
    let (_ta, fa, spa, ca) = setup("clref", &cfg);
    std::fs::write(&fa.hand, "").unwrap();
    assert_eq!(run_to_end(&spa, &ca, &["--plain"]).last(), Some(&0));
    let (ref_items, ref_cert) = decisions(&ca);
    let ref_stmt = std::fs::read(ca.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    // interrupted runs: SIGTERM at seeded random points, resume with the same command
    for seed in [7u64, 19] {
        let (_tb, fb, spb, cb) = setup(&format!("clint{seed}"), &cfg);
        std::fs::write(&fb.hand, "").unwrap();
        let mut x = seed;
        let mut codes = vec![];
        for round in 0..12 {
            let mut c = start(&spb, &cb, &["--plain"], &[]);
            if round < 4 {
                x = mix(x);
                let ms = 300 + x % 1700;
                std::thread::sleep(Duration::from_millis(ms));
                signal(&c, libc::SIGTERM);
            }
            let code = wait(&mut c, Duration::from_secs(300));
            codes.push(code);
            if code == 0 {
                break;
            }
        }
        assert_eq!(codes.last(), Some(&0), "{codes:?} {}", log_tail(&cb));
        assert!(codes.len() > 1, "no interrupt happened: {codes:?}");
        let (items, cert) = decisions(&cb);
        assert_eq!(items, ref_items, "seed {seed}: codes {codes:?}");
        assert_eq!(cert, ref_cert, "seed {seed}");
        let stmt = std::fs::read(cb.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
        assert!(stmt == ref_stmt, "seed {seed}: statements differ");
        assert_eq!(result_json(&cb)["verdict"], "closed");
        assert_eq!(verify(&cb), 0);
    }
}

#[test]
fn a_recipe_failure_is_a_structured_error_and_the_node_ends_open() {
    let _g = serial();
    let cfg = Toy {
        default_recipe: false,
        second_face: None,
        overrides: vec![(
            "R1".into(),
            [4, 1, 0],
            "NO_RELATION_IN_SEEDS".into(),
            vec![],
        )],
        ..Toy::default()
    };
    let (_t, files, sp, cdir) = setup("clfail", &cfg);
    std::fs::write(&files.hand, "").unwrap();
    // lane closeloop4 (closeloop.md 19): the uncovered key is a terminal PROPOSAL: the run stops for the operator
    // (exit 8); the operator rejects it and the node ends open (exit 10), as before
    let mut c = start(&sp, &cdir, &["--plain"], &[]);
    assert_eq!(
        wait(&mut c, Duration::from_secs(600)),
        ATTENTION,
        "{}",
        log_tail(&cdir)
    );
    assert_eq!(
        latest_proposals(&cdir, "toy").unwrap()["proposals"][0]["key"],
        serde_json::json!([4, 1, 0])
    );
    assert_eq!(resolve_terminals(&cdir, Some("toy"), None), 0);
    assert_eq!(run_to_end(&sp, &cdir, &["--plain"]).last(), Some(&10));
    let st = state(&cdir);
    assert_eq!(
        st.nodes["toy"].state.phase,
        NodePhase::Open,
        "{:?}",
        node_events(&cdir)
    );
    assert!(!node_events(&cdir).contains(&"declare_closed".to_string()));
    let r = result_json(&cdir);
    assert_eq!(r["verdict"], "open", "{r:#}");
    assert!(
        r["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b.as_str().unwrap().starts_with("open_keys")),
        "{r:#}"
    );
    let f = answers_lines(&cdir)
        .into_iter()
        .find(|v| v["key"] == serde_json::json!([4, 1, 0]))
        .expect("an answer for [4,1,0]");
    assert_eq!(f["verdict"], "NO_RELATION_IN_SEEDS");
    let fr = &f["failure_record"];
    assert_eq!(fr["schema"], "rustred.campaign.recipe-failure.v1");
    assert_eq!(fr["class"], "no_relation_in_seeds");
    assert_eq!(fr["recipe"]["id"], "R1");
    let rep = fr["reproduce"].as_str().unwrap();
    assert!(
        rep.contains(" solve-recipe --family ")
            && rep.contains("--key 4,1,0")
            && rep.contains("--recipe "),
        "{rep}"
    );
    assert_eq!(fr["key"], serde_json::json!([4, 1, 0]));
    // lane closeloop2 (closeloop.md 13.4): the statement's open list carries the recipe's per-key failure
    let open: Vec<Value> =
        std::fs::read_to_string(cdir.join("closeloop/toy/final/open_keys.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
    let o = open
        .iter()
        .find(|v| v["key"] == serde_json::json!([4, 1, 0]))
        .expect("[4,1,0] in the open list");
    let rf = &o["recipe_refusals"][0];
    assert_eq!(rf["recipe"], "R1", "{o}");
    assert_eq!(rf["class"], "no_relation_in_seeds", "{o}");
    assert!(
        rf["reproduce"].as_str().unwrap().contains("--key 4,1,0"),
        "{o}"
    );
    // the item whose only key failed escalated (never accepted)
    assert!(
        st.items
            .values()
            .any(|it| matches!(it.phase, ItemPhase::Escalated(_)))
    );
}

#[test]
fn lying_children_are_refused_and_never_close_the_node() {
    let _g = serial();
    // (a) a relation for another key (contract violation, row 22): attention after the attempts, nothing accepted
    let (_t, files, sp, cdir) = setup(
        "cllie1",
        &Toy {
            quirks: vec!["wrong_key".into()],
            ..Toy::default()
        },
    );
    std::fs::write(&files.hand, "").unwrap();
    // procfix5 (gate4 m3): the node ends open with items in attention: finished, not all closed = exit 10
    assert_eq!(
        run_to_end(&sp, &cdir, &["--plain"]).last(),
        Some(&FINISHED_OPEN)
    );
    assert_eq!(accepts(&cdir), 0, "nothing accepted from a lying child");
    let st = state(&cdir);
    assert_eq!(st.nodes["toy"].state.phase, NodePhase::Open);
    assert!(
        st.items
            .values()
            .any(|it| matches!(&it.phase, ItemPhase::Attention(c) if c == "solve_job_failed"))
    );
    let r = result_json(&cdir);
    assert!(
        r["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b.as_str().unwrap().starts_with("solve_items_in_attention")),
        "{r:#}"
    );
    // (b) a relation of another recipe digest than the pinned one: every R1 answer refused
    let (_t2, files2, sp2, cdir2) = setup(
        "cllie2",
        &Toy {
            r1_engine_digest: Some("0".repeat(64)),
            second_face: None,
            ..Toy::default()
        },
    );
    std::fs::write(&files2.hand, "").unwrap();
    // procfix5 (gate4 m3): the exit code follows the items at the end (default outcome: 0 closed, 10 not)
    let c2 = run_rejecting_proposals(&sp2, &cdir2);
    // lane closeloop3: the exit code follows the closing loop's verdict (outcome(): open -> 10, closed -> 0)
    let want = if result_json(&cdir2)["verdict"] == "closed" {
        0
    } else {
        10
    };
    assert_eq!(c2.last(), Some(&want), "{c2:?}");
    assert_eq!(accepts(&cdir2), 0);
    let ans = answers_lines(&cdir2);
    assert!(
        ans.iter()
            .any(|v| v["source"] == "R1" && v["verdict"] == "REFUSED"),
        "{ans:#?}"
    );
    assert_eq!(state(&cdir2).nodes["toy"].state.phase, NodePhase::Open);
    // (c) a relation that does not descend: refused by the core chain (not lower under the node comparator)
    let (_t3, files3, sp3, cdir3) = setup(
        "cllie3",
        &Toy {
            second_face: None,
            overrides: vec![("R1".into(), [4, 1, 0], "SOLVED".into(), vec![[5, 1, 0]])],
            ..Toy::default()
        },
    );
    std::fs::write(&files3.hand, "").unwrap();
    let c3 = run_rejecting_proposals(&sp3, &cdir3);
    // lane closeloop3: the exit code follows the closing loop's verdict (outcome(): open -> 10, closed -> 0)
    let want = if result_json(&cdir3)["verdict"] == "closed" {
        0
    } else {
        10
    };
    assert_eq!(c3.last(), Some(&want), "{c3:?}");
    let ans = answers_lines(&cdir3);
    let x = ans
        .iter()
        .find(|v| v["key"] == serde_json::json!([4, 1, 0]))
        .expect("answer");
    assert_eq!(x["verdict"], "REFUSED", "{x:#}");
    // the core chain refuses it (its `Rejected` verdict carries no per-key reason when NO key of the batch is certified:
    // request to the acceptance owner in the lane notes)
    assert!(
        x["failure"]["message"]
            .as_str()
            .unwrap()
            .contains("rejected"),
        "{x:#}"
    );
    assert_eq!(accepts(&cdir3), 0);
    assert_eq!(state(&cdir3).nodes["toy"].state.phase, NodePhase::Open);
    let _ = NodeEvent::Start;
}
