//! Lane procfix7, item (e) of the brief (gate6 letter (e) covered "interrupted = uninterrupted" with the SYNTHETIC source
//! only): interrupt a CLOSING-LOOP campaign (fake children) at random points DURING ITS ESCALATION ROUND, resume it to
//! the end, and compare the final statement with the uninterrupted reference; 20 seeds, under a CPU quota of at most
//! 40 %. Spawns processes and sends signals: only inside a private pid namespace (rule L10).
//!
//! IGNORED by default (about an hour). Run it inside a CPUQuota scope around the namespace, e.g.
//! `systemd-run --user --scope -q -p CPUQuota=40% -p MemoryMax=8G -p MemorySwapMax=0 -- nice -n 10 unshare -Urpf
//! --mount-proc -- <target>/debug/deps/procfix7_closeloop-<hash> --ignored --test-threads=1` (private TMPDIR); the test
//! REFUSES to run unless the cgroup it runs in (or an ancestor) has `cpu.max` <= 40 %.
//!
//! The node is the toy of closeloop_support (arity 3) without recipe sources, as in tests/t266run_fake.rs: the start
//! key's table rule hands [top,1,0] down, every key [k,1,0] below is uncovered and goes through the escalation
//! (symmetry at the key, per-key step recipes, recipe-solve jobs routed back, proofs), [1,1,0] ends as the terminal.

mod closeloop_support;
mod sup_common;

use closeloop_support::*;
use rustred_campaign::closeloop::derived::recipe_source_id;
use rustred_campaign::closeloop::ids::{KIND_FACTS, KIND_PROOF, KIND_SOLVE};
use rustred_campaign::state::{AttemptState, CampaignState};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};
use sup_common::*;

const STEPS: [&str; 2] = ["d0_cnone_foff_mM1C", "d1_cnone_foff_mM1C"];

fn in_ns() -> bool {
    if rustred_campaign::launch::in_private_namespace() {
        return true;
    }
    eprintln!(
        "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
    );
    false
}

/// The smallest `cpu.max` ratio (quota / period) of this process's cgroup and its ancestors (None: no quota).
fn cpu_quota() -> Option<f64> {
    let cg = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let rel = cg
        .lines()
        .find_map(|l| l.strip_prefix("0::"))?
        .trim()
        .to_string();
    let mut p = PathBuf::from("/sys/fs/cgroup").join(rel.trim_start_matches('/'));
    let mut best: Option<f64> = None;
    loop {
        if let Ok(t) = std::fs::read_to_string(p.join("cpu.max")) {
            let mut it = t.split_whitespace();
            if let (Some(q), Some(per)) = (it.next(), it.next())
                && let (Ok(q), Ok(per)) = (q.parse::<f64>(), per.parse::<f64>())
                && per > 0.0
            {
                let r = q / per;
                best = Some(best.map_or(r, |b: f64| b.min(r)));
            }
        }
        if p == Path::new("/sys/fs/cgroup") || !p.pop() {
            break;
        }
    }
    best
}

/// The toy without recipe sources, with the escalation configured (tests/t266run_fake.rs `setup`, big-node mode off).
fn setup(tag: &str, top: i32, ms: u64) -> (Tmp, PathBuf, PathBuf) {
    let t = Tmp::new(tag);
    let cfg = Toy {
        top,
        second_face: None,
        default_recipe: false,
        r1_from_two: false,
        table_evidence: true,
        ms,
        ..Toy::default()
    };
    let files = toy(&t.path.join("material"), &cfg, FAKE);
    std::fs::write(&files.hand, "").unwrap();
    let mut spec: Value =
        serde_json::from_slice(&std::fs::read(&files.import_spec).unwrap()).unwrap();
    let srcs: Vec<Value> = spec["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] != "recipe")
        .cloned()
        .collect();
    spec["sources"] = json!(srcs);
    std::fs::write(
        &files.import_spec,
        serde_json::to_vec_pretty(&spec).unwrap(),
    )
    .unwrap();
    let op = files.dir.join("oracle.jsonl");
    let mut o = String::new();
    for k in 2..=top {
        o.push_str(&format!(
            "{}\n",
            json!({"source": recipe_source_id(&[k, 1, 0], STEPS[0], &[true, true, false]), "key": [k, 1, 0],
                   "verdict": "SOLVED", "rhs": [[k - 1, 1, 0], [k, 0, 0]]})
        ));
    }
    std::fs::write(&op, o).unwrap();
    let mut c: Value = serde_json::from_slice(&std::fs::read(&files.config).unwrap()).unwrap();
    let node = &mut c["nodes"][0];
    node["hand"] = Value::Null;
    node["escalation"] =
        json!({"symmetry_at_key": true, "steps": STEPS, "terminals": {"max_terminals": 3}});
    node["budgets"]["max_rounds"] = json!(40);
    std::fs::write(&files.config, serde_json::to_vec_pretty(&c).unwrap()).unwrap();
    let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

fn result_json(cdir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(cdir.join("closeloop/toy/final/result.json")).unwrap())
        .unwrap()
}

fn statement(cdir: &Path) -> Vec<u8> {
    std::fs::read(cdir.join("closeloop/toy/final/statement/statement.jsonl")).unwrap()
}

fn terminals(cdir: &Path) -> Vec<u8> {
    std::fs::read(cdir.join("closeloop/toy/final/statement/terminals.json")).unwrap()
}

fn items(cdir: &Path) -> BTreeSet<(String, String, String)> {
    state(cdir)
        .items
        .iter()
        .map(|(k, it)| (k.clone(), it.kind.clone(), it.phase.tag().to_string()))
        .collect()
}

/// A job of an item of one of `kinds` is running.
fn running_kind(st: &CampaignState, kinds: &[&str]) -> bool {
    st.jobs.values().any(|j| {
        j.state == AttemptState::Running
            && st
                .items
                .get(&j.item)
                .is_some_and(|it| kinds.contains(&it.kind.as_str()))
    })
}

/// The escalation round is under way: its step recipes are recorded and one of its solves (or the proofs after it)
/// runs.
fn in_escalation(st: &CampaignState) -> bool {
    st.inputs.values().any(|r| r.role == "closeloop_recipes")
        && running_kind(st, &[KIND_SOLVE, KIND_PROOF, KIND_FACTS])
}

/// Poll the read-only ledger view until `pred` holds (true) or the supervisor exits / `limit` passes (false).
fn wait_for(
    cdir: &Path,
    c: &mut Child,
    limit: Duration,
    pred: impl Fn(&CampaignState) -> bool,
) -> bool {
    let t0 = Instant::now();
    while t0.elapsed() < limit {
        if c.try_wait().unwrap().is_some() {
            return false;
        }
        if cdir.join("ledger").join("MANIFEST.json").exists() && pred(&state(cdir)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

const KINDS: [&str; 5] = ["SIGINT", "SIGINT x2", "SIGTERM", "SIGKILL", "control stop"];

#[test]
#[ignore = "about an hour; run under a CPUQuota=40% scope (see the module documentation)"]
fn e_closing_loop_interrupted_during_escalation_equals_the_reference_20_seeds() {
    if !in_ns() {
        return;
    }
    let q = cpu_quota();
    assert!(
        q.is_some_and(|q| q <= 0.40 + 1e-9),
        "run under a CPU quota of at most 40 % (cpu.max of this cgroup or an ancestor; found {q:?})"
    );
    let _g = serial();
    let (_ta, spa, ca) = setup("pf7eref", 4, 1500);
    assert_eq!(
        run_to_end(&spa, &ca, &["--plain"]).last(),
        Some(&0),
        "{}",
        log_tail(&ca)
    );
    assert_eq!(result_json(&ca)["verdict"], "closed");
    let (ref_stmt, ref_terms, ref_items) = (statement(&ca), terminals(&ca), items(&ca));
    let mut per_kind = [0usize; 5];
    let mut total = 0usize;
    for seed in 1u64..=20 {
        let (_tb, spb, cb) = setup(&format!("pf7e{seed}"), 4, 1500);
        let mut x = mix(seed.wrapping_mul(0x9e37_79b9));
        let want = 1 + (x % 3) as usize; // 1 to 3 interruptions in this seed
        let mut done = 0usize;
        let mut codes = vec![];
        let mut kinds = vec![];
        for _session in 0..14 {
            let mut c = start(&spb, &cb, &["--plain"], &[]);
            if done < want && wait_for(&cb, &mut c, Duration::from_secs(900), in_escalation) {
                x = mix(x);
                std::thread::sleep(Duration::from_millis(x % 2500));
                x = mix(x);
                let k = (x % 5) as usize;
                match k {
                    0 => signal(&c, libc::SIGINT),
                    1 => {
                        signal(&c, libc::SIGINT);
                        std::thread::sleep(Duration::from_millis(200));
                        signal(&c, libc::SIGINT);
                    }
                    2 => signal(&c, libc::SIGTERM),
                    3 => signal(&c, libc::SIGKILL),
                    _ => {
                        let (rc, _) = cli(&["control", cb.to_str().unwrap(), "stop"]);
                        assert_eq!(rc, 0);
                    }
                }
                done += 1;
                kinds.push(KINDS[k]);
                per_kind[k] += 1;
            }
            let code = wait(&mut c, Duration::from_secs(900));
            codes.push(code);
            if code == 0 {
                break;
            }
            // an attempt of unknown execution after a crash: the operator's decision (as the user would)
            if code == ATTENTION {
                resolve_unknown(&cb);
            }
        }
        total += done;
        assert_eq!(
            codes.last(),
            Some(&0),
            "seed {seed}: codes {codes:?} after {kinds:?}: {}",
            log_tail(&cb)
        );
        assert!(
            done >= 1,
            "seed {seed}: no interruption happened during the escalation"
        );
        assert_eq!(result_json(&cb)["verdict"], "closed", "seed {seed}");
        assert!(
            statement(&cb) == ref_stmt,
            "seed {seed}: the final statement differs from the reference ({kinds:?}, codes {codes:?})"
        );
        assert!(terminals(&cb) == ref_terms, "seed {seed}: terminals differ");
        assert_eq!(
            items(&cb),
            ref_items,
            "seed {seed}: items differ ({kinds:?})"
        );
        assert_eq!(verify(&cb), 0, "seed {seed}");
        assert_no_marked(&_tb.campaign);
        eprintln!(
            "seed {seed}: {done} interruption(s) {kinds:?}, exit codes {codes:?}: statement == reference"
        );
    }
    eprintln!(
        "e: 20 seeds, {total} interruptions during the escalation round ({}), every final statement == reference; cpu quota {q:?}",
        KINDS
            .iter()
            .zip(per_kind)
            .map(|(k, n)| format!("{k} {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
}
