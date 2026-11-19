//! Lane wireflow (interfaces/repairflow.md 20): the repair flow's COMPOSITE step as CHILD JOBS under a REAL
//! `rr_campaign` supervisor, `rr_fake_child` emulating `rr_rf_symflow run` (plain relations over Q from the campaign
//! core; one of them relabelled as a TWO-ROW form by the script) and `rr_proof --symrows` (kernel certificates).
//! Test data: the T178 order-B frame table alone (read-only; skipped when absent) and eight arrival keys of the T178
//! arrival case (as tests/symcomp_fake.rs). Spawns processes and sends signals: run ONLY inside a private pid
//! namespace (`LIGHT_PIDNS=1 .../light_build.sh wireflow test -p rustred-campaign --test wireflow_fake`).
//!
//! Checked: (1) the rounds run as supervisor jobs (plan -> symmetry-composite job -> routed -> one proof-symrows job
//! per accepted relation -> round record with the sealed certificate files) and their relations are SERVED BY THE
//! IMPORT through its own load-time verification (plain = group re-derivation, two-row = kernel certificate), the same
//! checks as any supplied relation; (2) without the kernel the two-row form is NOT served (audit F8); (3) interrupt +
//! resume gives byte-identical round records, check record and statement; (4) the stage off (absent or explicitly
//! default) gives byte-identical results: the policy digest is unchanged and nothing of the stage appears.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod closeloop_support;
mod inputs;
mod sup_common;

use closeloop_support::{blake3_file, sha256_file};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use sup_common::*;

const WS: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev";

fn frame_table() -> Option<PathBuf> {
    let p = std::env::var_os("RUSTRED_SYMCOMP_FRAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(WS).join("SCRATCHPAD/t178/regenB/R0/try1/node/F.table.jsonl")
        });
    p.exists().then_some(p)
}

/// The eight arrival keys of tests/symcomp_fake.rs (4 plain relations in the fake child; key 0 is shipped as a
/// two-row form by the script below).
fn keys() -> Vec<Vec<i64>> {
    [
        "[1,1,1,1,1,1,4,1,1,5,0,0,0,0,0,0,0,0,0,0]",
        "[1,1,1,1,6,1,2,1,1,1,0,0,0,0,0,0,0,0,0,0]",
        "[1,1,1,1,1,1,1,1,1,3,0,-1,0,-1,-2,-3,0,0,0,0]",
        "[1,1,1,1,1,1,4,1,1,3,0,0,0,0,0,0,0,0,0,0]",
        "[1,1,2,1,1,2,1,1,1,9,0,0,0,0,0,0,0,0,0,0]",
        "[1,6,1,1,1,1,1,1,1,7,0,0,0,0,0,0,0,0,0,0]",
        "[1,1,1,1,9,1,4,1,1,1,0,0,0,0,0,0,0,0,0,0]",
        "[1,1,1,1,3,1,3,1,1,1,0,0,0,0,0,0,0,0,0,0]",
    ]
    .iter()
    .map(|s| serde_json::from_str(s).unwrap())
    .collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// composite jobs + the kernel
    Kernel,
    /// composite jobs, no kernel configured
    NoKernel,
    /// the stage absent (the explicit-default policy is written by its test)
    Off,
}

fn material(dir: &Path, frame: &Path, mode: Mode, ms: u64, rounds: u32) -> (PathBuf, PathBuf) {
    std::fs::create_dir_all(dir).unwrap();
    let start = dir.join("start.keys.jsonl");
    std::fs::write(
        &start,
        keys()
            .iter()
            .map(|k| format!("{}\n", serde_json::to_string(k).unwrap()))
            .collect::<String>(),
    )
    .unwrap();
    let spec = json!({"schema": "rustred.campaign.import-spec.v1", "node": "N", "frame_from": "F", "bits": 6,
        "start": {"path": start.display().to_string(), "format": "keys_jsonl"},
        "policies": {"tier": "K"}, "required_evidence": "imported",
        "sources": [{"id": "F", "kind": "engine_table", "class": "frame", "path": frame.display().to_string(),
            "sha256": sha256_file(frame), "removed": []}]});
    let import_spec = dir.join("import-spec.json");
    std::fs::write(&import_spec, serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
    let oracle = dir.join("oracle.jsonl");
    std::fs::write(&oracle, "").unwrap();
    let family = dir.join("N.family");
    std::fs::write(&family, "fake family (the fake child never reads it)\n").unwrap();
    let config = dir.join("closeloop.json");
    let cfg = json!({"schema":"rustred.campaign.closeloop.v1","nodes":[{
        "node":"N", "import_spec": import_spec.display().to_string(), "target":"boxed",
        "check_threads": 2, "default_recipe": null, "exact_tier": false,
        "budgets": {"max_rounds": 4, "max_job_attempts": 1},
        "solve": {"program": "rr_fake_child", "family": family.display().to_string(), "order": "presented",
                  "relation_rule": "first_admissible", "max_keys_per_job": 4, "threads": 1,
                  "cap_bytes": 268435456, "expected_peak_bytes": 67108864, "heartbeat_s": 1, "stop_grace_s": 3,
                  "fake": {"oracle": oracle.display().to_string(), "ms": ms, "program": "rr_fake_child"}}}]});
    std::fs::write(&config, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();
    let mut route0 = json!({"symmetry_credit": false, "symmetry_relation": false});
    match mode {
        Mode::Kernel | Mode::NoKernel => {
            route0["composite_jobs"] = json!({"enabled": true, "max_rounds": rounds, "program": "rr_fake_child",
                "wall_s": 900, "cap_bytes": 1073741824u64, "expected_peak_bytes": 268435456u64});
            if mode == Mode::Kernel {
                route0["composite_jobs"]["kernel"] = json!({"program": "rr_fake_child",
                    "sha256": sha256_file(Path::new(FAKE)), "census": "none", "wall_s": 600, "plain": true,
                    "cap_bytes": 1073741824u64, "expected_peak_bytes": 268435456u64});
            }
        }
        Mode::Off => {}
    }
    let rp = json!({"symbolic": {"enabled": false}, "recipes": {"enabled": false}, "route0": route0,
        "shadow_test": false, "proposals": {"declare_terminal": false}});
    let policy = dir.join("repairflow.json");
    std::fs::write(
        &policy,
        serde_json::to_vec_pretty(
            &json!({"schema":"rustred.campaign.repairflow.v1", "repair_policy": rp,
            "nodes": {},
            "fake": {"program": "rr_fake_child", "ms": ms, "faces": {},
                "composite_ops": [{"op": "prepare", "ms": ms},
                    {"op": "proof_answer", "ms": 0, "status": "two_row", "key": keys()[0]}],
                "kernel_ops": [{"op": "prepare", "ms": ms}]}}),
        )
        .unwrap(),
    )
    .unwrap();
    (config, policy)
}

fn setup(tag: &str, frame: &Path, mode: Mode, ms: u64) -> (Tmp, PathBuf, PathBuf) {
    // two rounds (a second round on the first round's new keys) only in the first test; one elsewhere (time)
    let rounds = if tag == "wfk" { 2 } else { 1 };
    let t = Tmp::new(tag);
    let (config, policy) = material(&t.path.join("material"), frame, mode, ms, rounds);
    let spec = json!({
        "schema": "rustred.campaign.spec.v1", "campaign": t.campaign,
        "programs": {"rr_fake_child": {"path": FAKE}},
        "resources": {"containment": "marker_only", "policy": "plain", "cpus": cpus(), "max_jobs": 4, "mem_total": "4G", "disk_floor": "1G",
                      "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M",
                      "start_timeout_s": 120},
        "progress": {"cadence_seconds": 1},
        "inputs": [{"id": "closeloop-config", "role": "closeloop", "path": config.display().to_string(),
                    "blake3": blake3_file(&config)},
                   {"id": "repairflow", "role": "repairflow", "path": policy.display().to_string(),
                    "blake3": blake3_file(&policy)}]
    });
    let sp = write_spec(&t, "spec.json", &spec);
    let cdir = t.cdir("cdir");
    (t, sp, cdir)
}

/// `sup_common::run_to_end` with a longer bound per session: two debug-built composite rounds of the fake child on a
/// saturated host take longer than its 900 s.
fn run_long(spec: &Path, cdir: &Path, extra: &[&str]) -> Vec<i32> {
    let mut codes = Vec::new();
    for _ in 0..6 {
        let mut c = start(spec, cdir, extra, &[]);
        let code = wait(&mut c, Duration::from_secs(3600));
        codes.push(code);
        if finished(code) {
            return codes;
        }
    }
    panic!(
        "campaign did not finish: exit codes {codes:?}; log {}",
        log_tail(cdir)
    );
}

/// A record (JSON, JSONL or text) with the campaign's temp root replaced by `<ROOT>` and every `spec_digest` member
/// removed (a digest over a spec text that names absolute paths of its own campaign).
fn norm_record(b: &[u8], cdir: &Path) -> String {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.remove("spec_digest");
                m.values_mut().for_each(strip);
            }
            Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let root = cdir.parent().unwrap_or(cdir).display().to_string();
    let text = String::from_utf8_lossy(b).replace(&root, "<ROOT>");
    match serde_json::from_str::<Value>(&text) {
        Ok(mut v) => {
            strip(&mut v);
            v.to_string()
        }
        Err(_) => text
            .lines()
            .map(|l| match serde_json::from_str::<Value>(l) {
                Ok(mut v) => {
                    strip(&mut v);
                    v.to_string()
                }
                Err(_) => l.to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .unwrap()
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

/// The final statement rows by key text.
fn statement(cdir: &Path) -> std::collections::BTreeMap<String, Value> {
    let t = std::fs::read_to_string(cdir.join("closeloop/N/final/statement/statement.jsonl"))
        .unwrap_or_default();
    t.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| {
            let k = v["key"].clone();
            let kt = match &k {
                Value::String(s) => s.clone(),
                Value::Array(a) => {
                    key_text(&a.iter().map(|x| x.as_i64().unwrap()).collect::<Vec<_>>())
                }
                _ => return None,
            };
            Some((kt, v))
        })
        .collect()
}

macro_rules! setup_or_skip {
    () => {{
        let Some(frame) = frame_table() else {
            inputs::missing(format_args!("SKIP: frame table absent"));
            return;
        };
        (serial(), frame)
    }};
}

#[test]
fn composite_rounds_run_as_jobs_and_their_relations_are_served_by_the_import() {
    let (_g, frame) = setup_or_skip!();
    let (_t, sp, cdir) = setup("wfk", &frame, Mode::Kernel, 20);
    let codes = run_long(&sp, &cdir, &["--plain"]);
    assert!(
        matches!(codes.last(), Some(&0) | Some(&10)),
        "{codes:?} {}",
        log_tail(&cdir)
    );
    let st = state(&cdir);
    // round 1: plan -> one symmetry-composite job (produced) -> routed -> kernel jobs -> the round record
    let plan = read_json(&cdir.join("closeloop/N/repairflow/composite/r1/plan.json"));
    let asked = plan["keys"].as_u64().unwrap();
    assert!(asked >= 4, "{plan:#}");
    let c1 = &st.items["N.rf.c1"];
    assert_eq!(c1.kind, "symmetry-composite");
    assert_eq!(c1.phase.tag(), "produced", "{}", log_tail(&cdir));
    let routed = read_json(&cdir.join("closeloop/N/repairflow/composite/r1/routed.json"));
    assert_eq!(routed["accepted"], 4, "{routed:#}");
    assert_eq!(routed["kernel_only"], 1, "{routed:#}");
    assert_eq!(routed["kernel"]["route"], "kernel", "{routed:#}");
    let jobs = routed["kernel"]["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 4, "{routed:#}");
    for j in jobs {
        let item = format!("N.rf.c1.k{}", j["j"]);
        assert_eq!(st.items[&item].kind, "proof-symrows");
        assert_eq!(st.items[&item].phase.tag(), "produced", "{item}");
    }
    let round = read_json(&cdir.join("closeloop/N/repairflow/composite/r1/round.json"));
    assert_eq!(round["certified"], 4, "{round:#}");
    assert_eq!(round["findings"], 0, "{round:#}");
    let files = round["composite_input"]["kernel_certificates"]["files"]
        .as_array()
        .unwrap();
    assert_eq!(files.len(), 4);
    assert_eq!(
        round["composite_input"]["kernel_certificates"]["kernels"],
        json!([sha256_file(Path::new(FAKE))])
    );
    // the stage ended (no new key, or the round cap) and P1 ran with the rounds' relations in its spec
    let done = read_json(&cdir.join("closeloop/N/repairflow/composite/done.json"));
    assert!(done["rounds"].as_u64().unwrap() >= 1, "{done:#}");
    let comp = read_json(&cdir.join("closeloop/N/repairflow/import-spec.composed.json"));
    let src = comp["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "rf:symmetry-composite:r1")
        .unwrap_or_else(|| panic!("{comp:#}"))
        .clone();
    assert_eq!(src["kind"], "supplied_relations");
    assert_eq!(
        src["kernel_certificates"]["files"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let check = read_json(&cdir.join("closeloop/N/repairflow/check.json"));
    let k0 = keys()[0].clone();
    for f in check["faces"].as_array().unwrap() {
        assert!(
            !f["open_keys"].as_array().unwrap().contains(&json!(k0)),
            "the two-row key is still open at P1: {f:#}"
        );
    }
    // the closing loop's statement: the four keys served by the round's source through the import's verification;
    // the two-row form by its kernel certificate
    let rows = statement(&cdir);
    let r0 = rows
        .get(&key_text(&k0))
        .unwrap_or_else(|| panic!("no statement row for the two-row key: {}", log_tail(&cdir)));
    let s0 = r0.to_string();
    assert!(s0.contains("rf:symmetry-composite:r1"), "{r0:#}");
    assert!(s0.contains("kernel_certified"), "{r0:#}");
    assert_eq!(verify(&cdir), 0);
}

#[test]
fn without_the_kernel_a_two_row_form_is_not_served() {
    let (_g, frame) = setup_or_skip!();
    let (_t, sp, cdir) = setup("wfn", &frame, Mode::NoKernel, 20);
    let codes = run_long(&sp, &cdir, &["--plain"]);
    assert!(
        matches!(codes.last(), Some(&0) | Some(&10)),
        "{codes:?} {}",
        log_tail(&cdir)
    );
    let routed = read_json(&cdir.join("closeloop/N/repairflow/composite/r1/routed.json"));
    assert_eq!(routed["accepted"], 4, "{routed:#}");
    assert_eq!(routed["kernel"]["route"], "same_backend", "{routed:#}");
    assert!(routed["kernel"]["jobs"].as_array().unwrap().is_empty());
    let round = read_json(&cdir.join("closeloop/N/repairflow/composite/r1/round.json"));
    assert!(
        round["composite_input"]["kernel_certificates"].is_null(),
        "{round:#}"
    );
    // F8: a two-row line never enters through the SAME-BACKEND route; without a kernel it has no certificate. (The
    // fake's "two-row" form is a relabelled ONE-element relation, so the import's own exact group re-derivation, route
    // (a), may serve it per-key: sound evidence of its own; a genuine two-row combination (a row at a shifted seed)
    // fails that route.)
    let rows = statement(&cdir);
    if let Some(r0) = rows.get(&key_text(&keys()[0])) {
        let t = r0.to_string();
        assert!(
            !t.contains("same_backend") && !t.contains("kernel_certified"),
            "{r0:#}"
        );
        if t.contains("rf:symmetry-composite") {
            assert!(t.contains("\"basis\":\"symmetry_relation\""), "{r0:#}");
        }
    }
    // the plain relations are served per-key (the import's own group re-derivation at load: `verify` = {})
    let r1 = rows
        .get(&key_text(&keys()[2]))
        .map(|v| v.to_string())
        .unwrap_or_default();
    assert!(r1.contains("rf:symmetry-composite:r1"), "{r1}");
    assert!(r1.contains("\"evidence\":\"per-key\""), "{r1}");
}

type Outcome = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

fn outcome(cdir: &Path) -> Outcome {
    let rd = |p: &str| std::fs::read(cdir.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
    (
        rd("closeloop/N/repairflow/composite/r1/round.json"),
        rd("closeloop/N/repairflow/composite/done.json"),
        rd("closeloop/N/repairflow/check.json"),
        rd("closeloop/N/final/statement/statement.jsonl"),
    )
}

#[test]
fn interrupt_and_resume_gives_the_same_rounds_and_statement() {
    let (_g, frame) = setup_or_skip!();
    let (_ta, spa, ca) = setup("wfref", &frame, Mode::Kernel, 150);
    let codes = run_long(&spa, &ca, &["--plain"]);
    assert!(matches!(codes.last(), Some(&0) | Some(&10)), "{codes:?}");
    let reference = outcome(&ca);
    let (_tb, spb, cb) = setup("wfint", &frame, Mode::Kernel, 150);
    let mut x = 17u64;
    let mut codes = vec![];
    for round in 0..16 {
        let mut c = start(&spb, &cb, &["--plain"], &[]);
        if round < 5 {
            x = mix(x);
            std::thread::sleep(Duration::from_millis(400 + x % 1500));
            signal(&c, libc::SIGTERM);
        }
        let code = wait(&mut c, Duration::from_secs(3600));
        codes.push(code);
        if code == 0 || code == 10 {
            break;
        }
    }
    assert!(
        matches!(codes.last(), Some(&0) | Some(&10)),
        "{codes:?} {}",
        log_tail(&cb)
    );
    assert!(codes.len() > 1, "no interrupt happened: {codes:?}");
    let o = outcome(&cb);
    // the records name paths of their own campaign (CDIR, material): compare with the temp roots normalised and the
    // digests over absolute-path texts (`spec_digest` of a round / P1 spec) removed
    let norm = |b: &[u8], d: &Path| norm_record(b, d);
    assert_eq!(
        norm(&o.0, &cb),
        norm(&reference.0, &ca),
        "round records differ"
    );
    assert_eq!(
        norm(&o.1, &cb),
        norm(&reference.1, &ca),
        "done records differ"
    );
    assert_eq!(
        norm(&o.2, &cb),
        norm(&reference.2, &ca),
        "check records differ"
    );
    assert_eq!(
        norm(&o.3, &cb),
        norm(&reference.3, &ca),
        "statements differ"
    );
    assert_eq!(verify(&cb), 0);
}

#[test]
fn the_stage_off_absent_or_explicit_gives_identical_results() {
    let (_g, frame) = setup_or_skip!();
    // ONE material directory (the import spec's paths enter the spec digest), two policy files, two campaigns
    let (t, spa, ca) = setup("wfoff", &frame, Mode::Off, 10);
    let mdir = t.path.join("material");
    let pol_a = mdir.join("repairflow.json");
    let mut pv: Value = serde_json::from_slice(&std::fs::read(&pol_a).unwrap()).unwrap();
    pv["repair_policy"]["route0"]["composite_jobs"] = json!({"enabled": false, "max_rounds": 4,
        "program": "rr_rf_symflow", "threads": 1, "max_attempts": 2, "kernel": null});
    pv["repair_policy"]["tworow"] = json!({"enabled": false});
    pv["repair_policy"]["pieces"] = json!({"plan": false});
    let pol_b = mdir.join("repairflow.explicit.json");
    std::fs::write(&pol_b, serde_json::to_vec_pretty(&pv).unwrap()).unwrap();
    let mut sv: Value = serde_json::from_slice(&std::fs::read(&spa).unwrap()).unwrap();
    for i in sv["inputs"].as_array_mut().unwrap() {
        if i["id"] == "repairflow" {
            i["path"] = json!(pol_b.display().to_string());
            i["blake3"] = json!(blake3_file(&pol_b));
        }
    }
    let spb = write_spec(&t, "spec.explicit.json", &sv);
    let cb = t.cdir("cdir-explicit");
    for (sp, c) in [(&spa, &ca), (&spb, &cb)] {
        let codes = run_long(sp, c, &["--plain"]);
        assert!(
            matches!(codes.last(), Some(&0) | Some(&10)),
            "{codes:?} {}",
            log_tail(c)
        );
        assert!(!c.join("closeloop/N/repairflow/composite").exists());
        assert!(!state(c).items.keys().any(|k| k.contains(".rf.c")));
    }
    let norm = |p: &str, d: &Path| {
        String::from_utf8_lossy(&std::fs::read(d.join(p)).unwrap())
            .replace(&d.display().to_string(), "<CDIR>")
    };
    for p in [
        "closeloop/N/repairflow/policy.json",
        "closeloop/N/repairflow/check.json",
        "closeloop/N/repairflow/decisions.json",
        "closeloop/N/repairflow/import-spec.composed.json",
        "closeloop/N/final/statement/statement.jsonl",
    ] {
        assert_eq!(norm(p, &ca), norm(p, &cb), "{p} differs");
    }
    // the effective policy record carries none of the new sections at their defaults (digest unchanged)
    let pol = norm("closeloop/N/repairflow/policy.json", &ca);
    assert!(
        !pol.contains("composite_jobs") && !pol.contains("tworow"),
        "{pol}"
    );
}

/// wireflow (c), repairflow.md 22: with `ship_regions` the composite rounds' region fits that are EXACT on a face's keys,
/// all certified, are shipped as case domains (export `regions/`, "verify per key at use"); every other face says why
/// not. This node's gate cannot run (Tier K), so the test sets `require_gate: false`; the gate rule itself is
/// unit-tested (closeloop::wire_regions). The round record keeps the per-face key lists the fit was made on.
#[test]
fn exact_region_fits_on_certified_keys_are_shipped_as_case_domains() {
    let (_g, frame) = setup_or_skip!();
    let (t, sp, cdir) = setup("wfreg", &frame, Mode::Kernel, 10);
    let pol = t.path.join("material/repairflow.json");
    let mut pv: Value = serde_json::from_slice(&std::fs::read(&pol).unwrap()).unwrap();
    pv["repair_policy"]["ship_regions"] = json!({"enabled": true, "require_gate": false});
    std::fs::write(&pol, serde_json::to_vec_pretty(&pv).unwrap()).unwrap();
    let mut sv: Value = serde_json::from_slice(&std::fs::read(&sp).unwrap()).unwrap();
    for i in sv["inputs"].as_array_mut().unwrap() {
        if i["id"] == "repairflow" {
            i["blake3"] = json!(blake3_file(&pol));
        }
    }
    let sp = write_spec(&t, "spec.regions.json", &sv);
    let codes = run_long(&sp, &cdir, &["--plain"]);
    assert!(
        matches!(codes.last(), Some(&0) | Some(&10)),
        "{codes:?} {}",
        log_tail(&cdir)
    );
    assert!(state(&cdir).inputs.contains_key("repairflow:N:regions"));
    let keys: Value = serde_json::from_slice(
        &std::fs::read(cdir.join("closeloop/N/repairflow/composite/r1/regions.keys.json")).unwrap(),
    )
    .unwrap();
    assert!(!keys["faces"].as_object().unwrap().is_empty(), "{keys:#}");
    let rec: Value = serde_json::from_slice(
        &std::fs::read(cdir.join("closeloop/N/repairflow/regions/regions.json")).unwrap(),
    )
    .unwrap();
    let faces = rec["faces"].as_array().unwrap();
    assert!(!faces.is_empty(), "{rec:#}");
    let mut shipped = 0;
    for f in faces {
        if f["shipped"] == true {
            shipped += 1;
            let file = f["file"].as_str().unwrap();
            let doc: Value = serde_json::from_slice(
                &std::fs::read(cdir.join("closeloop/N/repairflow/export").join(file)).unwrap(),
            )
            .unwrap();
            let cases = doc["case_domains"].as_array().unwrap();
            assert!(!cases.is_empty(), "{doc:#}");
            assert!(
                cases
                    .iter()
                    .all(|c| c["use"].as_str().unwrap().contains("per key at use"))
            );
            assert_eq!(doc["keys"], f["keys"]);
        } else {
            assert!(f["why"].is_string(), "{f:#}");
        }
    }
    assert_eq!(rec["shipped"], shipped);
    eprintln!("regions record: {rec:#}");
    let man: Value = serde_json::from_slice(
        &std::fs::read(cdir.join("closeloop/N/repairflow/export/manifest.json")).unwrap(),
    )
    .unwrap();
    assert!(man["regions"].is_object(), "{man:#}");
    assert_eq!(verify(&cdir), 0);
}
