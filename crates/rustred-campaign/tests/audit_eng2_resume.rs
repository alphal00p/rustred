//! Lane audit-eng2 (independent re-audit of the engine side of the child-job contract, binary of lane engbase2):
//! RANDOM interrupt / resume drills of the REAL child `rr_node job`, classified with the contract crate exactly as the
//! supervisor does (`classify::assess` + `classify::classify`), resumed from the classifier's own `resume_from`
//! (never from a hand-computed digest), and compared canonically with an uninterrupted run of the same spec.
//!
//! Each drill: spawn attempt 1; wait for a random target (the k-th unit event of the fixture, or a random time from
//! spawn, start-up included); sleep a random jitter; interrupt by one of: SIGTERM (graceful), SIGTERM then SIGINT
//! (second signal = hard-safe stop), SIGKILL (no OUTCOME), STOP file; classify (an interrupted attempt must accept
//! NOTHING); check that the committed prefix named by RESUME is a canonical prefix of the uninterrupted table; resume
//! (or restart from scratch when RESUME is not valid); with some probability interrupt the resumed attempt again
//! (double resume); run the last attempt to its end; compare exit code, stop code, decision row, canonical table, cert
//! and F.subcases digests with the uninterrupted run. Every difference is collected and reported at the end.
//!
//! All tests are `#[ignore]` and need: `RR_NODE_BIN` (rr_node with `job`), `RUSTRED_CAMPAIGN_FIXTURES`
//! (WS/fixtures/campaign-20261114), the licence environment (`source WS/ops/env.sh`), `LIGHT_PIDNS=1` (private pid
//! namespace: signals reach only our own children), `RR_REAL_OUT` (run directories), optional `RR_AUDIT_SEED`,
//! `RR_AUDIT_DRILLS` (drills per fixture), `RR_AUDIT_LOG` (one JSON line per drill).

mod audit_eng2_common;
use audit_eng2_common::*;

use rustred_campaign_contract::classify::{Accept, KillReason, WaitStatus};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::spec::JobSpec;
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

/// What the final attempt of a drill is compared on.
#[derive(Debug, Clone, PartialEq)]
struct Fin {
    exit: WaitStatus,
    stop: Option<String>,
    row: u8,
    table: Option<String>,
    cert: Option<String>,
    subcases: Option<String>,
}

fn fin(spec: &JobSpec, w: WaitStatus) -> Fin {
    let o = outcome(spec);
    Fin {
        exit: w,
        stop: o
            .as_ref()
            .and_then(|o| o.job.stop.code)
            .map(|c| format!("{c:?}")),
        row: classified(spec, w, KillReason::None).row,
        table: canonical_of(spec, "F.table.jsonl"),
        cert: canonical_of(spec, "F.cert.jsonl"),
        subcases: canonical_of(spec, "F.subcases.jsonl"),
    }
}

/// The uninterrupted reference: (final figures, canonical table lines, unit count, wall).
struct Reference {
    fin: Fin,
    lines: Vec<Vec<u8>>,
    units: usize,
    wall: Duration,
}

fn reference(r: &Path, fx: &Fx) -> Reference {
    let s = job_spec(r, &format!("{}-ref", fx.tag), 1, fx, 0);
    let t = Instant::now();
    let mut c = spawn(&s);
    let w = wait(&mut c);
    let wall = t.elapsed();
    let f = fin(&s, w);
    let lines =
        canonical_lines(&std::fs::read(RunLayout::new(&s.run_dir).table()).unwrap_or_default());
    let u = units(&events_of(&s), fx.numeric_units);
    eprintln!(
        "[{}] reference: {w:?} row {} stop {:?} in {wall:?}, {u} units, {} table lines",
        fx.tag,
        f.row,
        f.stop,
        lines.len()
    );
    Reference {
        fin: f,
        lines,
        units: u,
        wall,
    }
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Term,
    TermThenInt,
    Kill,
    StopFile,
}

/// One interruption of a running attempt. Returns (wait status, mode, target description, landed?).
fn interrupt(
    rng: &mut Rng,
    spec: &JobSpec,
    fx: &Fx,
    reference: &Reference,
    units_left: usize,
) -> (WaitStatus, Mode, String) {
    let mode = match rng.below(10) {
        0..=3 => Mode::Term,
        4..=5 => Mode::TermThenInt,
        6..=8 => Mode::Kill,
        _ => Mode::StopFile,
    };
    let mut c = spawn(spec);
    let t0 = Instant::now();
    let per_unit = reference.wall.as_millis() as u64 / (reference.units.max(1) as u64);
    let left = units_left.clamp(1, reference.units.max(1));
    let target = if rng.chance(0.2) {
        // a random time from spawn (start-up included), scaled to the work left
        let span = reference.wall.as_millis() as u64 * left as u64 / reference.units.max(1) as u64;
        let ms = rng.below(span + 1);
        format!("t+{ms}ms")
    } else {
        let k = 1 + rng.below(left as u64);
        let jitter = rng.below(per_unit.max(1) * 2 + 1);
        format!("unit{k}+{jitter}ms")
    };
    let limit = reference.wall * 3 + Duration::from_secs(60);
    if let Some(ms) = target.strip_prefix("t+").and_then(|x| x.strip_suffix("ms")) {
        let ms: u64 = ms.parse().unwrap();
        while t0.elapsed() < Duration::from_millis(ms) {
            if c.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    } else {
        let (k, jitter) = target
            .trim_start_matches("unit")
            .trim_end_matches("ms")
            .split_once('+')
            .map(|(a, b)| (a.parse::<usize>().unwrap(), b.parse::<u64>().unwrap()))
            .unwrap();
        loop {
            if units(&events_of(spec), fx.numeric_units) >= k || t0.elapsed() > limit {
                break;
            }
            if c.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        std::thread::sleep(Duration::from_millis(jitter));
    }
    match mode {
        Mode::Term => signal(&c, libc::SIGTERM),
        Mode::TermThenInt => {
            signal(&c, libc::SIGTERM);
            std::thread::sleep(Duration::from_millis(rng.below(150)));
            signal(&c, libc::SIGINT);
        }
        Mode::Kill => signal(&c, libc::SIGKILL),
        Mode::StopFile => {
            std::fs::write(
                RunLayout::new(&spec.run_dir).stop_file(),
                b"{\"reason\":\"audit\"}",
            )
            .unwrap();
        }
    }
    let w = wait(&mut c);
    (w, mode, target)
}

/// The committed prefix named by the attempt's RESUME must be a canonical prefix of the uninterrupted table.
fn prefix_check(spec: &JobSpec, reference: &Reference) -> Result<u64, String> {
    let layout = RunLayout::new(&spec.run_dir);
    let Ok(b) = std::fs::read(layout.resume()) else {
        return Ok(0);
    };
    let r: Value = serde_json::from_slice(&b).map_err(|e| format!("RESUME parse: {e}"))?;
    let bytes = r["table"]["bytes"].as_u64().unwrap_or(0) as usize;
    let table = std::fs::read(layout.table()).unwrap_or_default();
    if table.len() < bytes {
        return Err(format!(
            "RESUME names {bytes} table bytes, file has {}",
            table.len()
        ));
    }
    let lines = canonical_lines(&table[..bytes]);
    if lines.len() > reference.lines.len() || lines[..] != reference.lines[..lines.len()] {
        let k = lines
            .iter()
            .zip(&reference.lines)
            .position(|(a, b)| a != b)
            .unwrap_or(lines.len().min(reference.lines.len()));
        return Err(format!(
            "committed prefix ({} lines) is not a canonical prefix of the uninterrupted table: first difference at line {k}",
            lines.len()
        ));
    }
    Ok(r["rules_committed"].as_u64().unwrap_or(0))
}

fn log_line(v: &Value) {
    if let Ok(mut f) = std::env::var("RR_AUDIT_LOG").map_err(|_| ()).and_then(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .map_err(|_| ())
    }) {
        let _ = writeln!(f, "{v}");
    }
}

/// Run `drills` random drills of `fx`; return (interrupted attempts, differences).
fn drill(fx: &Fx, drills: u64, seed: u64) -> (u64, Vec<String>) {
    let r = root(fx.tag);
    let reference = reference(&r, fx);
    let mut rng = Rng(seed ^ 0x9E37_79B9_7F4A_7C15 ^ (fx.tag.len() as u64) << 7);
    let mut interrupted = 0u64;
    let mut diffs = Vec::new();
    for d in 0..drills {
        let every = fx.resume_every[rng.below(fx.resume_every.len() as u64) as usize];
        let job = format!("{}-d{d}", fx.tag);
        let mut spec = job_spec(&r, &job, 1, fx, every);
        let mut trail: Vec<Value> = Vec::new();
        let mut depth = 0;
        let mut left = reference.units;
        let last_w = loop {
            let double_ok = depth < 2 && (depth == 0 || rng.chance(0.45));
            if !double_ok {
                let mut c = spawn(&spec);
                break wait(&mut c);
            }
            let (w, mode, target) = interrupt(&mut rng, &spec, fx, &reference, left);
            // units this attempt started (the interrupted one is redone): what the next attempt has left at most
            left =
                left.saturating_sub(units(&events_of(&spec), fx.numeric_units).saturating_sub(1));
            if w == reference.fin.exit && w != WaitStatus::Exited(30) {
                // the attempt ended before the interruption took effect
                trail.push(json!({"attempt": spec.attempt, "mode": format!("{mode:?}"), "target": target, "landed": false}));
                break w;
            }
            interrupted += 1;
            let cl = classified(&spec, w, KillReason::Stop);
            let o = outcome(&spec);
            let graceful = o.as_ref().map(|o| o.job.stop.graceful);
            if cl.accept != Accept::Nothing {
                diffs.push(format!(
                    "[{job} a{}] interrupted attempt ({w:?}, {mode:?}) classified row {} with accept {:?}",
                    spec.attempt, cl.row, cl.accept
                ));
            }
            if matches!(w, WaitStatus::Exited(30)) && o.is_none() {
                diffs.push(format!("[{job} a{}] exit 30 without OUTCOME", spec.attempt));
            }
            if let WaitStatus::Exited(c) = w
                && c != 30
            {
                diffs.push(format!(
                    "[{job} a{}] interrupted by {mode:?} at {target} exited {c} (row {}, {})",
                    spec.attempt, cl.row, cl.why
                ));
            }
            let committed = match prefix_check(&spec, &reference) {
                Ok(n) => n,
                Err(m) => {
                    diffs.push(format!("[{job} a{}] {m}", spec.attempt));
                    0
                }
            };
            trail.push(json!({
                "attempt": spec.attempt, "mode": format!("{mode:?}"), "target": target, "landed": true,
                "wait": format!("{w:?}"), "row": cl.row, "graceful": graceful,
                "resume_valid": cl.resume_from.is_some(), "rules_committed": committed,
                "resume_every_s": every,
            }));
            spec = next_attempt(&r, &spec, cl.resume_from.clone());
            depth += 1;
        };
        let f = fin(&spec, last_w);
        let equal = f == reference.fin;
        if !equal {
            diffs.push(format!(
                "[{job}] final attempt {} differs from the uninterrupted run:\n  got  {f:?}\n  want {:?}",
                spec.attempt, reference.fin
            ));
        }
        if spec.attempt > 1
            && let Some(o) = outcome(&spec)
            && o.job.resumed != spec.resume.is_some()
        {
            diffs.push(format!(
                "[{job}] OUTCOME.resumed {} but spec.resume {}",
                o.job.resumed,
                spec.resume.is_some()
            ));
        }
        let line = json!({"fixture": fx.tag, "drill": d, "trail": trail, "final_attempt": spec.attempt,
                          "final_equal": equal, "final": format!("{f:?}")});
        eprintln!("{line}");
        log_line(&line);
    }
    (interrupted, diffs)
}

fn report(fx: &str, interrupted: u64, diffs: &[String]) {
    eprintln!(
        "[{fx}] interrupted attempts: {interrupted}; differences: {}",
        diffs.len()
    );
    for d in diffs {
        eprintln!("DIFF {d}");
    }
    log_line(&json!({"fixture": fx, "summary": true, "interrupted": interrupted, "diffs": diffs}));
}

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

const L3A_POINTS: [[i64; 9]; 8] = [
    [1, 1, 1, 1, 1, 1, 1, 1, 0],
    [2, 1, 1, 1, 1, 1, 1, 1, 0],
    [1, 2, 1, 1, 1, 1, 1, 1, 0],
    [1, 1, 1, 2, 1, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 1, 1, -1],
    [1, 1, 1, 1, 1, 2, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 1, 2, 0],
    [2, 1, 1, 1, 1, 1, 1, 1, -1],
];

fn fixture_case(node: &str, index: u64) -> Value {
    let p = fixtures()
        .join("small/m2-slice")
        .join(node)
        .join("table.jsonl");
    let text = std::fs::read_to_string(&p).unwrap();
    let v = text
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| v["type"] == "rule" && v["index"] == index)
        .unwrap_or_else(|| panic!("{node} rule {index}"));
    json!({"frame": "PRESENTED", "fixed": v["fixed"], "equations": v["equations"]})
}

fn run_fixture(fx: Fx, default_drills: u64) {
    let drills = env_u64("RR_AUDIT_DRILLS", default_drills);
    let seed = env_u64("RR_AUDIT_SEED", 20261114);
    let (n, diffs) = drill(&fx, drills, seed);
    report(fx.tag, n, &diffs);
    assert!(
        diffs.is_empty(),
        "{} differences (see DIFF lines)",
        diffs.len()
    );
}

/// Symbolic stage, small node (75 cases, ~0.8 s): many random landings, RESUME at every commit point or every 30 s.
#[test]
#[ignore]
fn audit_eng2_drill_l4a_symbolic() {
    run_fixture(
        Fx {
            tag: "l4a",
            node: "L4a",
            args: args(&["--order", "id"]),
            cases: vec![],
            resume_every: vec![0, 0, 30],
            stop_grace_s: 5,
            numeric_units: false,
        },
        16,
    );
}

/// The constructed numeric-endpoint job of lane engfix3 (L3a + 8 fixed ORIGINAL points, depth 0..3): landings in the
/// per-point loop, after the exhausted master, around the shared solve.
#[test]
#[ignore]
fn audit_eng2_drill_l3a_numeric() {
    let cases: Vec<Value> = L3A_POINTS
        .iter()
        .map(|p| json!({"frame": "ORIGINAL", "fixed": p, "equations": []}))
        .collect();
    run_fixture(
        Fx {
            tag: "l3anum",
            node: "L3a",
            args: args(&[
                "--order",
                "id",
                "--numeric-depth",
                "0",
                "--numeric-depth-max",
                "3",
            ]),
            cases,
            resume_every: vec![0, 0, 30],
            stop_grace_s: 5,
            numeric_units: true,
        },
        14,
    );
}

/// The symbolic stage of a larger node (T281-b6, 392 cases, ~100 s).
#[test]
#[ignore]
fn audit_eng2_drill_t281_symbolic() {
    run_fixture(
        Fx {
            tag: "t281",
            node: "T281-b6",
            args: args(&["--order", "id"]),
            cases: vec![],
            resume_every: vec![0, 1, 30],
            stop_grace_s: 5,
            numeric_units: false,
        },
        4,
    );
}

/// `--descend open` (own cases solved, returned cases never solved), incl. stops during the last own case.
#[test]
#[ignore]
fn audit_eng2_drill_t281_descend_open() {
    let cases: Vec<Value> = [363, 367, 370, 380]
        .iter()
        .map(|&i| fixture_case("T281-b6", i))
        .collect();
    run_fixture(
        Fx {
            tag: "t281desc",
            node: "T281-b6",
            args: args(&["--order", "id", "--descend", "open"]),
            cases,
            resume_every: vec![0, 30],
            stop_grace_s: 5,
            numeric_units: false,
        },
        6,
    );
}

/// `--open-on-refusal` with deterministic refusals (monomial caps; lane engfix3's fixture).
#[test]
#[ignore]
fn audit_eng2_drill_t281_open_on_refusal() {
    run_fixture(
        Fx {
            tag: "t281ref",
            node: "T281-b6",
            args: args(&[
                "--order",
                "id",
                "--open-on-refusal",
                "--max-rule-monomials",
                "400",
                "--fallback-rule-monomials",
                "400",
                "--no-refused-bank",
            ]),
            cases: vec![],
            resume_every: vec![0, 1],
            stop_grace_s: 5,
            numeric_units: false,
        },
        3,
    );
}

/// The numeric fixture interrupted INSIDE the shared solve (the stage after the per-point loop: phase
/// `shared_numeric`): SIGKILL, or SIGTERM + SIGINT, as soon as the phase event appears; the resumed attempt (RESUME of
/// safe point (d): no endpoint left, the exhausted master waiting) is interrupted again the same way (double resume);
/// the last attempt must equal the uninterrupted run.
#[test]
#[ignore]
fn audit_eng2_drill_l3a_shared_solve() {
    let cases: Vec<Value> = L3A_POINTS
        .iter()
        .map(|p| json!({"frame": "ORIGINAL", "fixed": p, "equations": []}))
        .collect();
    let fx = Fx {
        tag: "l3ashared",
        node: "L3a",
        args: args(&[
            "--order",
            "id",
            "--numeric-depth",
            "0",
            "--numeric-depth-max",
            "3",
        ]),
        cases,
        resume_every: vec![0],
        stop_grace_s: 5,
        numeric_units: true,
    };
    let r = root(fx.tag);
    let reference = reference(&r, &fx);
    let mut diffs = Vec::new();
    let mut landed = 0;
    for d in 0..env_u64("RR_AUDIT_DRILLS", 6) {
        let mut spec = job_spec(&r, &format!("sh{d}"), 1, &fx, 0);
        let mut trail = Vec::new();
        let mut ended: Option<WaitStatus> = None;
        for _round in 0..2 {
            let mut c = spawn(&spec);
            let t = Instant::now();
            let mut hit = false;
            while t.elapsed() < Duration::from_secs(60) {
                if c.try_wait().unwrap().is_some() {
                    break;
                }
                if events_of(&spec)
                    .iter()
                    .any(|v| v["kind"] == "phase" && v["phase"] == "shared_numeric")
                {
                    hit = true;
                    break;
                }
                std::thread::sleep(Duration::from_micros(300));
            }
            if d % 2 == 0 {
                signal(&c, libc::SIGKILL);
            } else {
                signal(&c, libc::SIGTERM);
                signal(&c, libc::SIGINT);
            }
            let w = wait(&mut c);
            if w == reference.fin.exit {
                trail.push(json!({"attempt": spec.attempt, "hit_phase": hit, "landed": false}));
                ended = Some(w);
                break;
            }
            landed += 1;
            let cl = classified(&spec, w, KillReason::Stop);
            if cl.accept != Accept::Nothing {
                diffs.push(format!(
                    "[sh{d} a{}] interrupted attempt accepted (row {})",
                    spec.attempt, cl.row
                ));
            }
            if let Err(m) = prefix_check(&spec, &reference) {
                diffs.push(format!("[sh{d} a{}] {m}", spec.attempt));
            }
            let res: Value = std::fs::read(RunLayout::new(&spec.run_dir).resume())
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or(Value::Null);
            trail.push(json!({"attempt": spec.attempt, "hit_phase": hit, "landed": true, "wait": format!("{w:?}"), "row": cl.row,
                              "numerical": res["numerical"].as_array().map(Vec::len),
                              "exhausted": res["engine"]["numeric_stage"]["exhausted"].as_array().map(Vec::len)}));
            spec = next_attempt(&r, &spec, cl.resume_from.clone());
        }
        let w = match ended {
            Some(w) => w,
            None => {
                let mut c = spawn(&spec);
                wait(&mut c)
            }
        };
        let f = fin(&spec, w);
        if f != reference.fin {
            diffs.push(format!("[sh{d}] final {f:?} != {:?}", reference.fin));
        }
        let line = json!({"fixture": fx.tag, "drill": d, "trail": trail, "final_equal": f == reference.fin});
        eprintln!("{line}");
        log_line(&line);
    }
    report(fx.tag, landed, &diffs);
    assert!(diffs.is_empty(), "{diffs:?}");
}
