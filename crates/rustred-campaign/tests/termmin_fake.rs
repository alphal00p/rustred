//! Lane termmin (interfaces/termmin.md 6): PHASE E of the closing loop under a REAL `rr_campaign` supervisor with
//! `rr_fake_child` emulating kind solve-recipe. Spawns processes and sends signals: run ONLY inside a private pid
//! namespace (`LIGHT_PIDNS=1 .../light_build.sh termmin test -p rustred-campaign --test termmin_fake`).
//!
//! The toy (termmin_support): closed at its first check with terminals M (no relation exists), E (eliminable at depth
//! 2), N (only through the new key X, served by the default recipe in the closing try).

mod sup_common;
mod termmin_support;

use rustred_campaign::ledger::{Event, ItemEvent};
use rustred_campaign::state::NodePhase;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use sup_common::*;
use termmin_support::*;

fn setup(tag: &str, t: &TmToy) -> (Tmp, TmFiles, PathBuf, PathBuf) {
    let tmp = Tmp::new(tag);
    let files = toy(&tmp.path.join("material"), t);
    let spec = campaign_spec(&tmp.campaign, &files, FAKE, &cpus());
    let sp = write_spec(&tmp, "spec.json", &spec);
    let cdir = tmp.cdir("cdir");
    (tmp, files, sp, cdir)
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .unwrap()
}

fn report(cdir: &Path) -> Value {
    read_json(&cdir.join("closeloop/toy/termmin/report.json"))
}

fn first_terminals(cdir: &Path) -> Value {
    read_json(&cdir.join("closeloop/toy/final/statement/terminals.json"))
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

fn outcomes(rep: &Value) -> Vec<(Value, String, String)> {
    rep["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["key"].clone(),
                t["outcome"].as_str().unwrap().to_string(),
                t["class"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn decisions(rep: &Value) -> Vec<Value> {
    rep["decisions"].as_array().unwrap().clone()
}

fn closed(cdir: &Path) {
    let st = state(cdir);
    assert_eq!(
        st.nodes["toy"].state.phase,
        NodePhase::Closed,
        "{:?}\n{}",
        node_events(cdir),
        log_tail(cdir)
    );
    let ev = node_events(cdir);
    assert_eq!(
        ev.last().map(String::as_str),
        Some("declare_closed"),
        "{ev:?}"
    );
    assert!(!ev.contains(&"declare_open".to_string()), "{ev:?}");
}

#[test]
fn phase_e_eliminates_two_of_three_terminals_and_writes_a_second_statement() {
    let _g = serial();
    let (_t, _f, sp, cdir) = setup("tmfull", &TmToy::default());
    let codes = run_to_end(&sp, &cdir, &["--plain"]);
    assert_eq!(codes.last(), Some(&0), "{}", log_tail(&cdir));
    closed(&cdir);
    // the first statement: three terminals, untouched
    let ft = first_terminals(&cdir);
    assert_eq!(ft["terminals"].as_array().map(Vec::len), Some(3), "{ft:#}");
    let rep = report(&cdir);
    assert_eq!(
        outcomes(&rep),
        vec![
            (json!(N), "eliminated".into(), "eliminated".into()),
            (json!(E), "eliminated".into(), "eliminated".into()),
            (json!(M), "kept".into(), "no_relation_in_seeds".into()),
        ],
        "{rep:#}"
    );
    assert_eq!(rep["final_terminals"], json!([M]));
    assert_eq!(rep["economy_before"]["terminals"], 3);
    assert_eq!(rep["economy_after"]["terminals"], 1);
    // N's elimination went through the closing try (X served by the default recipe): the closure grew by X
    assert!(
        rep["economy_after"]["closure_keys"].as_u64()
            >= rep["economy_before"]["closure_keys"].as_u64(),
        "{rep:#}"
    );
    // M: every step recorded with its seed set size
    let m = &rep["terminals"][2];
    assert!(
        m["steps"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["outcome"] != "pending"),
        "{m:#}"
    );
    assert!(m["steps"][3]["seeds"].as_u64().unwrap() > 0);
    // the second statement beside the first: closed, one terminal
    let ss = &rep["second_statement"];
    assert_eq!(ss["written"], true, "{ss:#}");
    assert_eq!(ss["closed"], true, "{ss:#}");
    assert_eq!(ss["terminals"], json!(["I(1,1,0)"]), "{ss:#}");
    assert!(
        cdir.join("closeloop/toy/termmin/statement/statement.jsonl")
            .is_file()
    );
    // lane closeloop3 (closeloop.md 16.7): phase E's relations got span proofs of the closing loop's proof pass (items
    // <n>.p0.<j>, accepted) and the second statement's served keys are per-key (recipe relations with bound proofs)
    let st = state(&cdir);
    let p0: Vec<_> = st
        .items
        .iter()
        .filter(|(k, it)| k.starts_with("toy.p0.") && it.kind == "proof-span")
        .collect();
    assert!(!p0.is_empty(), "no phase-E relation proof item");
    assert!(
        p0.iter().all(|(_, it)| it.phase.tag() == "accepted" && it.label == "span:relation"),
        "{p0:?}"
    );
    let second = std::fs::read_to_string(cdir.join("closeloop/toy/termmin/statement/statement.jsonl")).unwrap();
    for l in second.lines() {
        let v: Value = serde_json::from_str(l).unwrap();
        if v["status"] == "served" {
            assert_eq!(v["evidence"], "per-key", "{v}");
        }
    }
    assert!(
        cdir.join("closeloop/toy/final/statement/statement.jsonl")
            .is_file()
    );
    // decisions are ledger inputs (one per decision)
    let st = state(&cdir);
    let n_dec = st
        .inputs
        .keys()
        .filter(|k| k.starts_with("closeloop:toy:tm:decision:"))
        .count();
    assert_eq!(n_dec, decisions(&rep).len());
    assert_eq!(verify(&cdir), 0, "progress verify");
}

#[test]
fn a_preferred_terminal_is_never_attempted() {
    let _g = serial();
    let t = TmToy {
        termmin: Some(
            json!({"schema": "rustred.campaign.termmin.v1", "sequence": SEQUENCE, "preferred": [E]}),
        ),
        ..TmToy::default()
    };
    let (_t, _f, sp, cdir) = setup("tmpref", &t);
    assert_eq!(
        run_to_end(&sp, &cdir, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&cdir)
    );
    closed(&cdir);
    let st = state(&cdir);
    assert!(
        st.items
            .values()
            .all(|it| !(it.kind == "termmin-solve" && it.label.contains("I(2,1,0)"))),
        "no solve for the preferred terminal"
    );
    let rep = report(&cdir);
    let e = rep["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["key"] == json!(E))
        .unwrap()
        .clone();
    assert_eq!(e["outcome"], "preferred", "{e:#}");
    assert_eq!(rep["final_terminals"], json!([M, E]), "{rep:#}");
}

#[test]
fn failing_attempts_leave_the_closed_verdict_and_the_first_statement_untouched() {
    let _g = serial();
    // reference: phase E disabled
    let off = TmToy {
        termmin: Some(json!({"schema": "rustred.campaign.termmin.v1", "enabled": false})),
        ..TmToy::default()
    };
    let (_ta, _fa, spa, ca) = setup("tmoff", &off);
    assert_eq!(run_to_end(&spa, &ca, &["--plain"]).last(), Some(&0));
    closed(&ca);
    assert!(!ca.join("closeloop/toy/termmin/report.json").exists());
    // every attempt fails
    let t = TmToy {
        no_relations: true,
        x_unservable: true,
        ..TmToy::default()
    };
    let (_tb, _fb, spb, cb) = setup("tmfail", &t);
    assert_eq!(
        run_to_end(&spb, &cb, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&cb)
    );
    closed(&cb);
    let a = std::fs::read(ca.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    let b = std::fs::read(cb.join("closeloop/toy/final/statement/statement.jsonl")).unwrap();
    assert!(a == b, "the first statement does not depend on phase E");
    let ra = read_json(&ca.join("closeloop/toy/final/result.json"));
    let rb = read_json(&cb.join("closeloop/toy/final/result.json"));
    assert_eq!(ra["verdict"], "closed");
    assert_eq!(rb["verdict"], "closed");
    let rep = report(&cb);
    assert_eq!(rep["final_terminals"], json!([M, N, E]), "{rep:#}");
    assert_eq!(rep["second_statement"]["written"], false);
    assert!(
        outcomes(&rep).iter().all(|(_, o, _)| o == "kept"),
        "{rep:#}"
    );
    // every kept terminal lists its attempts with a reason
    for t in rep["terminals"].as_array().unwrap() {
        assert!(
            t["steps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["outcome"] == "no_relation"),
            "{t:#}"
        );
    }
}

#[test]
fn a_relation_bringing_an_unservable_key_is_discarded() {
    let _g = serial();
    let t = TmToy {
        x_unservable: true,
        ..TmToy::default()
    };
    let (_t, _f, sp, cdir) = setup("tmunserv", &t);
    assert_eq!(
        run_to_end(&sp, &cdir, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&cdir)
    );
    closed(&cdir);
    let rep = report(&cdir);
    assert_eq!(
        outcomes(&rep),
        vec![
            (json!(N), "kept".into(), "brings_unserved_keys".into()),
            (json!(E), "eliminated".into(), "eliminated".into()),
            (json!(M), "kept".into(), "no_relation_in_seeds".into()),
        ],
        "{rep:#}"
    );
    assert_eq!(rep["final_terminals"], json!([M, N]));
}

#[test]
fn lying_children_are_refused() {
    let _g = serial();
    // (a) a relation for ANOTHER key (every record of every termmin job): nothing accepted, all kept, still closed
    let t = TmToy {
        termmin: Some(
            json!({"schema": "rustred.campaign.termmin.v1", "sequence": SEQUENCE,
                             "fake_quirks": ["wrong_key"]}),
        ),
        ..TmToy::default()
    };
    let (_t, _f, sp, cdir) = setup("tmwrong", &t);
    assert_eq!(
        run_to_end(&sp, &cdir, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&cdir)
    );
    closed(&cdir);
    assert_eq!(
        count(
            &cdir,
            |e| matches!(e, Event::Item { item, ev: ItemEvent::Accept { .. } } if item.contains(".tm."))
        ),
        0,
        "nothing accepted from a lying child"
    );
    let rep = report(&cdir);
    assert_eq!(rep["final_terminals"], json!([M, N, E]), "{rep:#}");
    // (b) a NON-DESCENDING relation (E onto a higher key at depth 2): refused by the accept chain; depth 3 is tried
    let t = TmToy {
        overrides: vec![
            (
                "termmin/d2_clast_fon".into(),
                E,
                "SOLVED".into(),
                vec![[3, 1, 0]],
            ),
            (
                "termmin/d3_cfirst_foff".into(),
                E,
                "SOLVED".into(),
                vec![M, [2, 0, 0]],
            ),
        ],
        ..TmToy::default()
    };
    let (_t2, _f2, sp2, cdir2) = setup("tmdesc", &t);
    assert_eq!(
        run_to_end(&sp2, &cdir2, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&cdir2)
    );
    closed(&cdir2);
    let rep = report(&cdir2);
    let e = rep["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["key"] == json!(E))
        .unwrap()
        .clone();
    assert_eq!(e["outcome"], "eliminated", "{e:#}");
    assert_eq!(e["steps"][3]["outcome"], "refused", "{e:#}");
    assert_eq!(e["steps"][4]["outcome"], "eliminated", "{e:#}");
}

#[test]
fn interrupt_and_resume_gives_the_same_final_set() {
    let _g = serial();
    let t = TmToy {
        ms: 300,
        ..TmToy::default()
    };
    let (_ta, _fa, spa, ca) = setup("tmref", &t);
    assert_eq!(run_to_end(&spa, &ca, &["--plain"]).last(), Some(&0));
    let ref_rep = report(&ca);
    for seed in [5u64, 23] {
        let (_tb, _fb, spb, cb) = setup(&format!("tmint{seed}"), &t);
        let mut x = seed;
        let mut codes = vec![];
        for round in 0..16 {
            let mut c = start(&spb, &cb, &["--plain"], &[]);
            if round < 5 {
                x = mix(x);
                std::thread::sleep(Duration::from_millis(400 + x % 2500));
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
        closed(&cb);
        let rep = report(&cb);
        assert_eq!(
            rep["final_terminals"], ref_rep["final_terminals"],
            "seed {seed}"
        );
        assert_eq!(decisions(&rep), decisions(&ref_rep), "seed {seed}");
        assert_eq!(outcomes(&rep), outcomes(&ref_rep), "seed {seed}");
        assert_eq!(verify(&cb), 0);
    }
}
