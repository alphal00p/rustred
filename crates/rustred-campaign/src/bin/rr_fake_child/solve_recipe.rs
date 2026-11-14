//! `rr_fake_child` emulating kind `solve-recipe` (lane adapters; interfaces/adapters.md 1.1, coord-recipe-solve.md 7):
//! an L1 child that reads the job's `keys` input, writes one `relation` record per key (from the script's `relation`
//! ops, in order), publishes `out/results.jsonl` once at the end (written in `tmp/`, sealed with its end record,
//! renamed), writes OUTCOME (exit 0 every key SOLVED, 10 `keys_unsolved`, 30 interrupted with a sealed `partial`) and
//! the contract's EVENTS envelope. Signals: the first stops at the next key boundary (the running key finishes), the
//! second or `stop_grace_s` interrupts at once. `recipe_quirk` ops emulate deviations of a real child (refusal tests).
//!
//! Independence: records, the sealed stream and OUTCOME are built with this binary's own code (`own`), as the rest of
//! the fake child; the contract crate's verifier classifies them (a differential test of the verifier).

use super::{
    LAST_SIG, PROGRAM, SIGNALS, alloc_touch, git_commit, own, refuse_exit, sigkill_self, source_of,
    status_of, vmhwm_kb,
};
use rustred_campaign_contract::classify::recipe::{FAILURE_SCHEMA, RELATION_TYPE, Verdict};
use rustred_campaign_contract::events::{Event, EventSink, Phase};
use rustred_campaign_contract::fake::{FakeOp, FaultKind, RecipeQuirk};
use rustred_campaign_contract::layout::{RESULTS, RunLayout};
use rustred_campaign_contract::preflight::{ProgramSelf, preflight};
use rustred_campaign_contract::spec::{JobKind, JobSpec};
use rustred_campaign_contract::stop::{StopCode, StopSource};
use rustred_campaign_contract::{ExitCode, JobStatus};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::Ordering::SeqCst;
use std::time::{Duration, Instant};

/// Features of the fake child in this mode.
const FEATURES: [&str; 2] = ["fake", "solve_recipe"];
/// The fake's determinism id (a real verb writes its own).
pub const DETERMINISM_ID: &str = "rustred.fake-recipe-solve.v1";

fn hex(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

fn key_text(k: &[i64]) -> String {
    format!(
        "I({})",
        k.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

struct Run {
    spec: JobSpec,
    spec_blake3: String,
    layout: RunLayout,
    sink: EventSink,
    keys: Vec<(Vec<i64>, Option<String>)>,
    recipe_digest: String,
    quirks: BTreeSet<RecipeQuirk>,
    started: Instant,
    jitter: bool,
}

impl Run {
    /// 0 = run, 1 = graceful stop requested (finish the running key), 2 = interrupt now.
    fn stop_level(&self) -> u32 {
        let n = SIGNALS.load(SeqCst);
        let stopfile = self.layout.stop_file().exists();
        match (n, stopfile) {
            (0, false) => 0,
            (0, true) | (1, _) => 1,
            _ => 2,
        }
    }

    fn stop_source(&self) -> StopSource {
        if SIGNALS.load(SeqCst) > 0 {
            source_of(LAST_SIG.load(SeqCst))
        } else {
            StopSource::StopFile
        }
    }

    /// Sleep `ms` (scaled), polling every 50 ms; returns the stop level reached. A graceful request lets the running
    /// key finish; the second signal or `stop_grace_s` after the first ends the wait at once.
    fn work(&self, ms: u64, scale: f64, first_stop: &mut Option<Instant>) -> u32 {
        let end = Instant::now() + Duration::from_secs_f64(ms as f64 * scale / 1000.0);
        loop {
            let lvl = self.stop_level();
            if lvl >= 1 && first_stop.is_none() {
                *first_stop = Some(Instant::now());
                let _ = self.sink.emit(Event::StopRequested {
                    source: self.stop_source(),
                    count: SIGNALS.load(SeqCst).max(1),
                });
            }
            let grace_over = first_stop
                .is_some_and(|t| t.elapsed() >= Duration::from_secs(self.spec.stop_grace_s as u64));
            if lvl >= 2 || grace_over {
                return 2;
            }
            let now = Instant::now();
            if now >= end {
                return lvl;
            }
            std::thread::sleep((end - now).min(Duration::from_millis(50)));
        }
    }

    fn failure(&self, key: &[i64], verdict: Verdict) -> Value {
        let class = verdict.class().map(|c| c.name()).unwrap_or_default();
        let recipe = self.spec.inputs.iter().find(|i| i.role == "recipe");
        let family = self.spec.inputs.iter().find(|i| i.role == "family");
        let k = key
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",");
        json!({
            "schema": FAILURE_SCHEMA, "class": class,
            "message": format!("fake: {} at {}", verdict.name(), key_text(key)),
            "detail": {},
            "node": {"family": family.map(|f| f.path.clone()), "family_blake3": family.map(|f| f.blake3.clone())},
            "recipe": {"id": "fake", "digest": self.recipe_digest, "path": recipe.map(|r| r.path.clone())},
            "key": key, "determinism_id": DETERMINISM_ID,
            "binary": {"path": PROGRAM, "blake3": self.spec.program.blake3},
            "reproduce": format!("{PROGRAM} solve-recipe --key {k}"),
            "context": {"request_key": null, "rule_chain": []},
        })
    }

    fn record(&self, ordinal: usize, verdict: Verdict, rhs: &[Vec<i64>], unproved: bool) -> Value {
        let (key, tag) = &self.keys[ordinal];
        let (sec, kb) = if self.jitter {
            (
                self.started.elapsed().as_secs_f64(),
                vmhwm_kb() + ordinal as u64,
            )
        } else {
            (0.0, 0)
        };
        let mut r = json!({
            "type": RELATION_TYPE, "ordinal": ordinal, "key": key, "tag": tag, "verdict": verdict.name(),
            "determinism_id": DETERMINISM_ID, "recipe_digest": self.recipe_digest, "target": key_text(key),
            "seconds": sec, "vmhwm_kb": kb,
        });
        if verdict == Verdict::Solved {
            let rhs_v: Vec<Value> = rhs
                .iter()
                .map(|k| json!([key_text(k), "1/(d-4)", "top"]))
                .collect();
            let digest = hex(format!("fake relation|{key:?}|{rhs:?}").as_bytes());
            r["rhs"] = Value::Array(rhs_v);
            r["relation_digest"] = Value::from(digest);
            r["rhs_count"] = Value::from(rhs.len());
            r["all_lower"] = Value::Bool(true);
            r["proof"] = json!({"kind": "exact-weights", "ok": !unproved, "weights": rhs.len()});
            r["failure"] = Value::Null;
        } else {
            r["rhs"] = Value::Null;
            r["relation_digest"] = Value::Null;
            r["failure"] = self.failure(key, verdict);
        }
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn outcome(
        &self,
        code: ExitCode,
        stop: Option<StopCode>,
        source: Option<StopSource>,
        graceful: bool,
        extra: Option<Value>,
        accepted: u64,
        time_decisions: Vec<Value>,
        error: Option<(&str, String)>,
    ) -> Vec<u8> {
        let digest = if self.quirks.contains(&RecipeQuirk::PlainSpecDigest) {
            hex(&std::fs::read(self.layout.job_spec()).unwrap_or_default())
        } else {
            self.spec_blake3.clone()
        };
        let effective = if self.quirks.contains(&RecipeQuirk::EffectiveWithoutDigest) {
            json!({"subcommand": "solve-recipe", "args": self.spec.args})
        } else {
            Value::Null
        };
        let td = !time_decisions.is_empty();
        let job = json!({
            "schema": rustred_campaign_contract::OUTCOME_SCHEMA,
            "job_id": self.spec.job_id, "attempt": self.spec.attempt, "kind": "solve-recipe",
            "spec_blake3": digest, "status": status_of(code), "exit_code": code.code(),
            "stop": {"code": stop, "detail": null, "source": source, "graceful": graceful},
            "program": {"name": PROGRAM, "version": env!("CARGO_PKG_VERSION"), "git_commit": git_commit(),
                        "features": FEATURES},
            "effective": effective,
            "seed": {"sample_seed": 0, "point_source": "seed"},
            "inputs": [], "counters": {"cases": self.keys.len(), "rules_committed": accepted},
            "budgets": {"work": {}, "wall": self.spec.budgets.wall, "mem_soft_bytes": null, "hit": null},
            "time_decisions": time_decisions, "time_decided": td, "per_case": [],
            "outputs": {"extra": extra.map(|e| vec![e]).unwrap_or_default()},
            "timing": {}, "memory": {},
            "error": error.map(|(c, m)| json!({"class": c, "message": m})),
            "resumed": false,
        });
        let o = json!({"complete": code == ExitCode::Complete, "rules": accepted, "job": job});
        let mut v = serde_json::to_vec_pretty(&o).unwrap();
        v.push(b'\n');
        v
    }

    fn finished(&self, code: ExitCode, stop: Option<StopCode>) {
        let _ = self.sink.emit(Event::JobFinished {
            exit_code: code,
            status: JobStatus::for_exit(code),
            stop_code: stop,
        });
    }
}

/// Entry point of the mode (the spec's kind is solve-recipe and it carries a fake script).
pub fn run(spec_path: &Path) -> ! {
    let me = ProgramSelf {
        name: PROGRAM,
        features: &FEATURES,
        kinds: &[JobKind::SolveRecipe],
        blake3: None,
    };
    let prep = match preflight(spec_path, &me, &|k| std::env::var(k).ok()) {
        Ok(p) => p,
        Err(r) => refuse_exit(r),
    };
    let spec = prep.spec.clone();
    let script = spec
        .fake
        .clone()
        .expect("validated: an emulated kind has a script");
    let layout = prep.layout.clone();
    let input = |role: &str| {
        spec.inputs
            .iter()
            .find(|i| i.role == role)
            .map(|i| std::fs::read(spec.input_path(i)).unwrap_or_default())
            .unwrap_or_default()
    };
    // the keys input, parsed by this program's own code (`[ints]` or {"key":[..],"tag":..})
    let mut keys = Vec::new();
    for l in String::from_utf8_lossy(&input("keys")).lines() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).unwrap_or(Value::Null);
        let (k, t) = match &v {
            Value::Array(_) => (v.clone(), None),
            _ => (v["key"].clone(), v["tag"].as_str().map(str::to_string)),
        };
        match serde_json::from_value::<Vec<i64>>(k) {
            Ok(k) => keys.push((k, t)),
            Err(e) => {
                eprintln!("rr_fake_child: keys input: {e}");
                std::process::exit(ExitCode::Input.code());
            }
        }
    }
    for d in [layout.out_dir(), layout.tmp_dir()] {
        if let Err(e) = std::fs::create_dir_all(&d) {
            eprintln!("rr_fake_child: {}: {e}", d.display());
            std::process::exit(ExitCode::Io.code());
        }
    }
    let sink = match EventSink::create(&layout.events()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rr_fake_child: EVENTS: {e}");
            std::process::exit(ExitCode::Io.code());
        }
    };
    let quirks: BTreeSet<RecipeQuirk> = script
        .script
        .iter()
        .filter_map(|op| match op {
            FakeOp::RecipeQuirk { quirk } => Some(*quirk),
            _ => None,
        })
        .collect();
    let r = Run {
        recipe_digest: hex(&input("recipe")),
        spec,
        spec_blake3: prep.spec_blake3.clone(),
        layout,
        sink,
        keys,
        quirks,
        started: Instant::now(),
        jitter: script.volatile_jitter,
    };
    let _ = r.sink.emit(Event::JobStarted {
        job_id: r.spec.job_id.clone(),
        attempt: r.spec.attempt,
        spec_blake3: r.spec_blake3.clone(),
        pid: std::process::id(),
        program: PROGRAM.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        git_commit: git_commit(),
        resume_from: None,
    });
    let scale = script.time_scale;
    let mut first_stop: Option<Instant> = None;
    let mut interrupted = false;
    let mut hard = false;
    let mut records: Vec<Value> = Vec::new();
    let mut next = 0usize;
    let mut unproved_done = false;
    let mut time_decisions: Vec<Value> = Vec::new();
    let mut end_seen = false;
    for (k, op) in script.script.iter().enumerate() {
        for f in script.faults.iter().filter(|f| f.at_op == k) {
            match f.kind {
                FaultKind::SigkillSelf => sigkill_self(),
                FaultKind::Abort => std::process::abort(),
                FaultKind::Exit => std::process::exit(f.code),
                FaultKind::Panic => panic!("fake panic at op {k}"),
                _ => {}
            }
        }
        if interrupted {
            break;
        }
        match op {
            FakeOp::Prepare { ms } => {
                let _ = r.sink.emit(Event::Phase {
                    phase: Phase::Prepare,
                });
                if r.work(*ms, scale, &mut first_stop) >= 1 {
                    interrupted = true;
                    hard = r.stop_level() >= 2;
                }
            }
            FakeOp::Heartbeats { n } => {
                let ms = r.spec.heartbeat_s as u64 * *n as u64 * 1000;
                if r.work(ms, 1.0, &mut first_stop) >= 1 {
                    interrupted = true;
                }
            }
            FakeOp::Alloc { mb } => alloc_touch(*mb << 20),
            FakeOp::RecipeQuirk { .. } => {}
            FakeOp::Relation { ms, verdict, rhs } => {
                if next >= r.keys.len() {
                    eprintln!("rr_fake_child: more relation ops than input keys");
                    std::process::exit(ExitCode::Internal.code());
                }
                if next == 0 {
                    let _ = r.sink.emit(Event::Phase {
                        phase: Phase::CaseSearch,
                    });
                }
                let lvl = r.work(*ms, scale, &mut first_stop);
                if lvl >= 2 {
                    interrupted = true;
                    hard = true;
                    break;
                }
                let unproved = *verdict == Verdict::Solved
                    && r.quirks.contains(&RecipeQuirk::UnprovedSolved)
                    && !unproved_done;
                unproved_done |= unproved;
                let mut rec = r.record(next, *verdict, rhs, unproved);
                if next == 0 && r.quirks.contains(&RecipeQuirk::WrongKey) {
                    if let Some(x) = rec["key"].get_mut(0) {
                        *x = Value::from(x.as_i64().unwrap_or(0) + 1);
                    }
                }
                if *verdict == Verdict::BudgetWall
                    && !r.quirks.contains(&RecipeQuirk::NoTimeDecision)
                {
                    let budget = r.spec.budgets.wall.case_s.unwrap_or(1) as f64;
                    time_decisions.push(
                        json!({"what": "case_wall", "case_ordinal": next, "rule": null,
                        "limit_s": budget, "measured_s": budget + 1.0, "taken": "budget_wall"}),
                    );
                }
                let _ = r.sink.emit(Event::Engine {
                    event: json!({"RecipeKey": {"unique": next, "verdict": verdict.name()}}),
                });
                records.push(rec);
                next += 1;
                if lvl == 1 {
                    interrupted = true;
                }
            }
            FakeOp::End { .. } => {
                end_seen = true;
                break;
            }
            FakeOp::Fail { code, message } => {
                let b = r.outcome(
                    *code,
                    Some(match code {
                        ExitCode::Input => StopCode::InputError,
                        ExitCode::Io => StopCode::IoError,
                        ExitCode::Internal => StopCode::InternalError,
                        _ => StopCode::Refused,
                    }),
                    None,
                    true,
                    None,
                    0,
                    vec![],
                    Some((
                        match code {
                            ExitCode::Input => "input",
                            ExitCode::Io => "io",
                            ExitCode::Internal => "internal",
                            ExitCode::Usage => "usage",
                            _ => "refused",
                        },
                        message.clone(),
                    )),
                );
                let _ = own::write_new(&r.layout.outcome(), &b);
                r.finished(*code, None);
                std::process::exit(code.code());
            }
            _ => {
                eprintln!("rr_fake_child: op not available in kind solve-recipe: {op:?}");
                std::process::exit(ExitCode::Usage.code());
            }
        }
    }
    if !interrupted && !end_seen && next < r.keys.len() {
        eprintln!("rr_fake_child: the script ended before every input key had a record");
        std::process::exit(ExitCode::Internal.code());
    }
    if !interrupted && end_seen && next < r.keys.len() {
        eprintln!("rr_fake_child: `end` before every input key had a record");
        std::process::exit(ExitCode::Internal.code());
    }
    // the remaining keys of an interrupted batch
    while records.len() < r.keys.len() {
        records.push(r.record(records.len(), Verdict::Interrupted, &[], false));
    }
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for x in &records {
        *counts
            .entry(x["verdict"].as_str().unwrap_or("?").to_string())
            .or_default() += 1;
    }
    let accepted = records
        .iter()
        .filter(|x| x["verdict"] == "SOLVED" && x["proof"]["ok"] == true)
        .count() as u64;
    let end_type = if interrupted { "partial" } else { "end" };
    let end = json!({"type": end_type, "records": records.len(), "counts": counts,
                     "recipe_digest": r.recipe_digest, "determinism_id": DETERMINISM_ID});
    // L1: written in tmp/, sealed, then published by one rename
    let tmp = r.layout.tmp_dir().join(RESULTS);
    let fig = (|| -> std::io::Result<own::Figures> {
        let mut w = own::Writer::create(&tmp)?;
        for x in &records {
            w.append(&serde_json::to_vec(x).unwrap())?;
        }
        let f = w.seal(&serde_json::to_vec(&end).unwrap())?;
        std::fs::rename(&tmp, r.layout.results())?;
        Ok(f)
    })();
    let fig = match fig {
        Ok(f) => f,
        Err(e) => {
            eprintln!("rr_fake_child: results: {e}");
            std::process::exit(ExitCode::Io.code());
        }
    };
    for f in script
        .faults
        .iter()
        .filter(|f| f.at_op == script.script.len())
    {
        match f.kind {
            FaultKind::KillBeforeOutcome | FaultKind::SigkillSelf => sigkill_self(),
            FaultKind::Abort => std::process::abort(),
            FaultKind::Exit => std::process::exit(f.code),
            _ => {}
        }
    }
    let mut extra = json!({"role": "results", "path": RunLayout::out_rel(RESULTS), "bytes": fig.bytes,
        "blake3": fig.blake3, "records": fig.records, "canonical_blake3": fig.canonical, "end": end_type,
        "sealed": true});
    if r.quirks.contains(&RecipeQuirk::NoCanonicalDigest) {
        extra.as_object_mut().unwrap().remove("canonical_blake3");
    }
    let all_solved = counts.keys().all(|v| v == "SOLVED");
    let (code, stop, source, graceful) = if interrupted {
        (
            ExitCode::Interrupted,
            Some(StopCode::Interrupted),
            Some(r.stop_source()),
            !hard,
        )
    } else if all_solved && accepted == records.len() as u64 {
        (ExitCode::Complete, None, None, true)
    } else {
        (
            ExitCode::Incomplete,
            Some(StopCode::KeysUnsolved),
            None,
            true,
        )
    };
    let b = r.outcome(
        code,
        stop,
        source,
        graceful,
        Some(extra),
        accepted,
        time_decisions,
        None,
    );
    if let Err(e) = own::write_new(&r.layout.outcome(), &b) {
        eprintln!("rr_fake_child: OUTCOME: {e}");
        std::process::exit(ExitCode::Io.code());
    }
    r.finished(code, stop);
    std::process::exit(code.code())
}
