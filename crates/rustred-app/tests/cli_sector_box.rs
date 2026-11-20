//! `rustred sector-box-reduce`: the CLI against golden eta-derivative
//! reductions produced by the campaign driver of the sector-box engine.
//!
//! Default tests (seconds): the three-loop K6 vacuum family with one massive
//! line deformed by `eta` (2 dimension values, 12 masters), checkpoint
//! resume, the certificate-prime rule, a two-loop family at arity 3 and the
//! input-error contract. Ignored: the four-loop prism family at 68 dimension
//! values; see `prism_4l_eta_derivative_equations_match_the_campaign`.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sector_box");

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("rustred-sector-box-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs the CLI on `spec` (written into `dir`); returns the exit code and
/// the report.
fn run(dir: &Path, spec: &Value) -> (i32, Value) {
    let spec_path = dir.join("spec.json");
    std::fs::write(&spec_path, spec.to_string()).unwrap();
    let report_path = dir.join("cli-report.json");
    let output = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(["sector-box-reduce", "--spec"])
        .arg(&spec_path)
        .arg("--output")
        .arg(&report_path)
        .arg("--force")
        .output()
        .unwrap();
    let code = output.status.code().unwrap();
    let report = std::fs::read_to_string(&report_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    if code != 0 && report.is_null() {
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    (code, report)
}

/// The exchange-format content that must be identical: parameters, fixed
/// values, coefficient field, ordered masters and every rule as a map
/// integral -> coefficient string (stats, timings, provenance and the family
/// block may differ).
fn content(
    path: &Path,
) -> (
    Value,
    Vec<Value>,
    BTreeMap<String, BTreeMap<String, String>>,
) {
    let solution: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(solution["schema"], "rustred-harness-solution.v1");
    let head = json!([
        solution["parameters"],
        solution["fixed"],
        solution["coefficient_field"]
    ]);
    let masters = solution["masters"].as_array().unwrap().clone();
    let rules = solution["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rule| {
            let terms = rule["terms"]
                .as_array()
                .unwrap()
                .iter()
                .map(|term| {
                    (
                        term["integral"].to_string(),
                        term["coefficient"].as_str().unwrap().to_string(),
                    )
                })
                .collect();
            (rule["target"].to_string(), terms)
        })
        .collect();
    (head, masters, rules)
}

fn assert_same_solution(produced: &Path, golden: &Path) {
    let (head, masters, rules) = content(produced);
    let (golden_head, golden_masters, golden_rules) = content(golden);
    assert_eq!(head, golden_head, "{}", produced.display());
    assert_eq!(masters, golden_masters, "{}", produced.display());
    assert_eq!(rules.len(), golden_rules.len(), "{}", produced.display());
    assert!(
        rules == golden_rules,
        "rules differ: {}",
        produced.display()
    );
}

/// Every verification prime and the certificate prime lie outside the
/// reconstruction primes of the solution.
fn assert_independent_primes(per_d: &Value) {
    let primes: Vec<u64> = serde_json::from_value(per_d["reconstruction_primes"].clone()).unwrap();
    let verification: Vec<u64> =
        serde_json::from_value(per_d["verification_primes"].clone()).unwrap();
    assert!(!primes.is_empty() && !verification.is_empty());
    assert!(verification.iter().all(|prime| !primes.contains(prime)));
    let certificate = per_d["certificates"]["probabilistic"]["prime"]
        .as_u64()
        .unwrap();
    assert!(!primes.contains(&certificate) && !verification.contains(&certificate));
}

fn k6_spec(dir: &Path) -> Value {
    json!({
        "schema": "rustred.sector-box-reduce-spec.v1",
        "family": format!("{FIXTURES}/k6/family.toml"),
        "out_dir": dir.join("out"),
        "targets": [[1, 1, 1, 1, 1, 1]],
        "preferred": [[1, 1, 1, 1, 1, 1]],
        "derivative_slot": 0,
        "closure_rounds": 8,
        "d_list": ["319999/80000", "37/11"],
        "router": "census-top",
        "seeding": {"kind": "staircase", "s_pad": 0, "d_pad": 0, "max_growth": 0},
        "master_check": "report",
        "lp_gate": false,
        "workers": 2,
        "max_primes": 64,
        "checkpoint_dir": dir.join("ckpt"),
    })
}

/// Three-loop K6 eta family: the same 12 masters and exactly the same
/// reductions (closed under the eta derivative) as the campaign driver at
/// two dimension values; a second run resumes from the checkpoint and
/// reproduces them.
#[test]
fn k6_eta_derivative_equations_match_the_campaign_goldens() {
    let dir = scratch("k6");
    let spec = k6_spec(&dir);
    let (code, report) = run(&dir, &spec);
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["status"], "ok");
    assert_eq!(report["restored_from_checkpoint"], false);
    assert_eq!(report["modular"]["masters"].as_array().unwrap().len(), 12);
    let closure = report["modular"]["closure"].as_array().unwrap();
    assert_eq!(closure.last().unwrap()["added"], 0);
    assert!(closure.len() > 1, "the closure added derivative seeds");
    for value in ["319999_80000", "37_11"] {
        let name = format!("solution-{value}.json");
        assert_same_solution(
            &dir.join("out").join(&name),
            &Path::new(FIXTURES).join("k6").join(&name),
        );
    }
    for per_d in report["per_d"].as_array().unwrap() {
        assert_eq!(per_d["ok"], true);
        assert_independent_primes(per_d);
    }
    // Resume: the modular phase comes from the checkpoint, the probes from
    // the probe log; the solutions are the same.
    let (code, again) = run(&dir, &spec);
    assert_eq!(code, 0, "{again}");
    assert_eq!(again["restored_from_checkpoint"], true);
    assert_eq!(again["targets"], report["targets"]);
    assert!(again["per_d"][0]["resumed_probes"].as_u64().unwrap() > 0);
    for value in ["319999_80000", "37_11"] {
        let name = format!("solution-{value}.json");
        assert_same_solution(
            &dir.join("out").join(&name),
            &Path::new(FIXTURES).join("k6").join(&name),
        );
    }
    // Another request (other seeding) does not reuse the checkpoint.
    let mut other = spec.clone();
    other["seeding"]["max_growth"] = json!(1);
    other["d_list"] = json!(["37/11"]);
    let (code, third) = run(&dir, &other);
    assert_eq!(code, 0, "{third}");
    assert_eq!(third["restored_from_checkpoint"], false);
    // A reconstruction prime as the certificate prime is refused: the
    // certificate fails, exit 1, and the solution is kept for diagnosis as
    // `solution-37_11.json.rejected`; the certified `solution-37_11.json`
    // of the earlier run is not replaced.
    let good = std::fs::read(dir.join("out").join("solution-37_11.json")).unwrap();
    let mut reused = spec.clone();
    reused["d_list"] = json!(["37/11"]);
    reused["certificate_prime"] = json!(
        report["per_d"][1]["reconstruction_primes"][0]
            .as_u64()
            .unwrap()
    );
    let (code, verdict) = run(&dir, &reused);
    assert_eq!(code, 1, "{verdict}");
    assert_eq!(verdict["status"], "certificate-failed");
    assert_eq!(
        verdict["per_d"][0]["certificates"]["probabilistic"]["ok"],
        false
    );
    assert_eq!(
        verdict["per_d"][0]["certificates"]["probabilistic"]["verdict"],
        "no-verdict"
    );
    assert_eq!(verdict["per_d"][0]["file"], "solution-37_11.json.rejected");
    assert!(
        dir.join("out")
            .join("solution-37_11.json.rejected")
            .exists()
    );
    assert_eq!(
        std::fs::read(dir.join("out").join("solution-37_11.json")).unwrap(),
        good
    );
    // A solution-*.json glob of the output directory sees only certified
    // solutions, never the rejected one.
    let globbed = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with("solution-") && name.ends_with(".json"))
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        globbed(&dir.join("out")),
        vec!["solution-319999_80000.json", "solution-37_11.json"]
    );
    // A later certified run of the same value supersedes the rejected file.
    let mut certified = reused.clone();
    certified["certificate_prime"] = Value::Null;
    let (code, again) = run(&dir, &certified);
    assert_eq!(code, 0, "{again}");
    assert!(
        !dir.join("out")
            .join("solution-37_11.json.rejected")
            .exists()
    );
    assert_eq!(again["per_d"][0]["file"], "solution-37_11.json");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Two loops at arity 3 (a sunrise with one deformed line): the finite-field
/// solution passes the probabilistic and the exact certificate.
#[test]
fn two_loop_sunrise_passes_both_certificates() {
    let dir = scratch("sunrise");
    std::fs::write(
        dir.join("family.toml"),
        r#"schema = "rustred.project.toml.v1"
parameters = ["d", "eta"]

[family]
name = "sunrise_eta"
loop_momenta = ["k1", "k2"]
external_momenta = []
dimension = "d"

[[family.denominators]]
id = "D1"
expression = "k1^2-1-eta"

[[family.denominators]]
id = "D2"
expression = "k2^2-1"

[[family.denominators]]
id = "D3"
expression = "(k1+k2)^2-1"
"#,
    )
    .unwrap();
    let spec = json!({
        "schema": "rustred.sector-box-reduce-spec.v1",
        "family": dir.join("family.toml"),
        "out_dir": dir.join("out"),
        "targets": [[1, 1, 1], [2, 1, 1], [1, 2, 1]],
        "derivative_slot": 0,
        "d_list": ["7/2"],
        "seeding": {"kind": "staircase", "s_pad": 0, "d_pad": 1, "max_growth": 1},
        "lp_gate": false,
        "exact_certificate": true,
    });
    let (code, report) = run(&dir, &spec);
    assert_eq!(code, 0, "{report}");
    let per_d = &report["per_d"][0];
    assert_eq!(per_d["certificates"]["exact"]["ok"], true);
    assert_eq!(per_d["certificates"]["probabilistic"]["ok"], true);
    assert_independent_primes(per_d);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Invalid requests are input errors (exit 4) with no panic; the report
/// says why when the request got far enough to write one.
#[test]
fn invalid_requests_are_input_errors() {
    let dir = scratch("invalid");
    let base = k6_spec(&dir);
    let mut cases: Vec<(&str, Value)> = Vec::new();
    let mut with = |label: &'static str, edit: &dyn Fn(&mut Value)| {
        let mut spec = base.clone();
        edit(&mut spec);
        cases.push((label, spec));
    };
    with("schema", &|s| {
        s["schema"] = json!("rustred.sector-box-reduce-spec.v0")
    });
    with("unknown field", &|s| s["bogus"] = json!(1));
    with("missing family", &|s| {
        s["family"] = json!("/nonexistent/family.toml")
    });
    with("target arity", &|s| s["targets"] = json!([[1, 1, 1]]));
    with("no targets", &|s| s["targets"] = json!([]));
    with("index range", &|s| {
        s["targets"] = json!([[1, 1, 1, 1, 1, 40000]])
    });
    with("empty d_list", &|s| s["d_list"] = json!([]));
    with("zero denominator", &|s| s["d_list"] = json!(["1/0"]));
    with("not a rational", &|s| s["d_list"] = json!(["x"]));
    with("two slashes", &|s| s["d_list"] = json!(["1/2/3"]));
    with("one value twice", &|s| s["d_list"] = json!(["1/2", "2/4"]));
    with("derivative slot", &|s| s["derivative_slot"] = json!(6));
    with("top sector", &|s| s["top_sector"] = json!(64));
    with("router", &|s| s["router"] = json!("global"));
    with("order", &|s| s["order"] = json!("dots-first"));
    with("master check", &|s| s["master_check"] = json!("loose"));
    with("seeding kind", &|s| {
        s["seeding"] = json!({"kind": "hull", "r": 1})
    });
    with("workers", &|s| s["workers"] = json!(0));
    with("free parameter", &|s| s["free"] = json!("d"));
    with("fixed unknown", &|s| s["fixed"] = json!({"m": "1"}));
    with("expected id", &|s| s["expected"] = json!({"top": 1}));
    with("tubes without staircase", &|s| {
        s["seeding"] = json!({"kind": "kira-box", "r": 8, "s": 0, "d": 2});
        s["tubes"] = json!({"min_rank": 1, "halo": 0});
    });
    // The report in `out_dir` is never left at "running": a request that
    // fails after its first write ends with status "failed" and the error.
    let out_report =
        |spec: &Value| PathBuf::from(spec["out_dir"].as_str().unwrap()).join("report.json");
    for (label, spec) in cases {
        let _ = std::fs::remove_file(out_report(&spec));
        let (code, report) = run(&dir, &spec);
        assert_eq!(code, 4, "{label}: {report}");
        if let Ok(text) = std::fs::read_to_string(out_report(&spec)) {
            let written: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(written["status"], "failed", "{label}: {written}");
            assert_eq!(written["error"]["kind"], "input", "{label}: {written}");
        } else {
            assert_ne!(
                label, "index range",
                "the index check runs after the first write"
            );
        }
    }
    // An arity without a compiled dispatch (2 denominators).
    std::fs::write(
        dir.join("two.toml"),
        r#"schema = "rustred.project.toml.v1"
[family]
name = "two"
loop_momenta = ["k1", "k2"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "D1"
expression = "k1^2-1"
[[family.denominators]]
id = "D2"
expression = "k2^2-1"
"#,
    )
    .unwrap();
    let mut spec = base.clone();
    spec["family"] = json!(dir.join("two.toml"));
    spec["targets"] = json!([[1, 1]]);
    let (code, _) = run(&dir, &spec);
    assert_eq!(code, 4);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Four-loop prism family (arity 10), 68 dimension values: every solution
/// equals the campaign's. Needs the campaign reference solutions:
/// `RUSTRED_SECTOR_BOX_PRISM_REFERENCE=<dir of solution-*.json>`
/// `cargo test -p rustred-app --release --test cli_sector_box -- --ignored prism`
/// (about 15 s at 2 workers).
#[test]
#[ignore = "four loops; needs RUSTRED_SECTOR_BOX_PRISM_REFERENCE"]
fn prism_4l_eta_derivative_equations_match_the_campaign() {
    let reference = PathBuf::from(
        std::env::var("RUSTRED_SECTOR_BOX_PRISM_REFERENCE")
            .expect("set RUSTRED_SECTOR_BOX_PRISM_REFERENCE to the reference solution directory"),
    );
    let dir = scratch("prism");
    let mut spec: Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(FIXTURES).join("prism4l/spec.json")).unwrap(),
    )
    .unwrap();
    spec["family"] = json!(format!("{FIXTURES}/prism4l/family.toml"));
    spec["out_dir"] = json!(dir.join("out"));
    let (code, report) = run(&dir, &spec);
    assert_eq!(code, 0, "{report}");
    let values = spec["d_list"].as_array().unwrap();
    assert_eq!(values.len(), 68);
    for (value, per_d) in values.iter().zip(report["per_d"].as_array().unwrap()) {
        let name = format!(
            "solution-{}.json",
            value.as_str().unwrap().replace('/', "_").replace('-', "m")
        );
        assert_same_solution(&dir.join("out").join(&name), &reference.join(&name));
        assert_independent_primes(per_d);
        assert_eq!(per_d["ok"], true);
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The default certificate prime is a prime outside the reconstruction
/// primes and distinct from the structure prime.
#[test]
fn default_certificate_prime_is_independent() {
    let prime = rustred_app::DEFAULT_CERTIFICATE_PRIME;
    assert!(symbolica::domains::integer::Integer::from(prime).is_prime(0));
    let list = rustred::algebra::reconstruct::reconstruction_primes(4096);
    assert!(!list.contains(&prime));
    assert_ne!(prime, (1 << 61) - 1);
}

#[test]
fn solution_names_are_file_safe() {
    assert_eq!(
        rustred_app::solution_file_name("319999/80000"),
        "solution-319999_80000.json"
    );
    assert_eq!(
        rustred_app::solution_file_name("-7/2"),
        "solution-m7_2.json"
    );
}

/// `report_supports` (cluster probes): `supports.jsonl` lists every demanded
/// integral once with the masters of its reduced row; every support entry is
/// a master, and a demanded master is its own support.
#[test]
fn supports_list_the_masters_of_each_demanded_integral() {
    let dir = scratch("supports");
    let spec = json!({
        "schema": "rustred.sector-box-reduce-spec.v1",
        "family": format!("{FIXTURES}/p3l_no/family.toml"),
        "out_dir": dir.join("out"),
        "targets": [[1, 1, 1, 1, 1, 1, 1, 1, 0], [1, 1, 1, 1, 1, 1, 1, 2, 0]],
        "derivative_slots": [0],
        "d_list": ["37/11"],
        "free": "z",
        "router": "identity",
        "lp_gate": false,
        "seeding": {"kind": "staircase", "s_pad": 0, "d_pad": 0, "max_growth": 3},
        "workers": 2,
        "modular_only": true,
        "report_supports": true,
    });
    let (code, report) = run(&dir, &spec);
    assert_eq!(code, 0, "{report}");
    let masters: BTreeSet<Vec<i64>> =
        serde_json::from_value(report["modular"]["masters"].clone()).unwrap();
    let text = std::fs::read_to_string(dir.join("out").join("supports.jsonl")).unwrap();
    let mut targets = BTreeSet::new();
    for line in text.lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        let target: Vec<i64> = serde_json::from_value(entry["target"].clone()).unwrap();
        let support: Vec<Vec<i64>> = serde_json::from_value(entry["support"].clone()).unwrap();
        assert!(targets.insert(target.clone()), "{target:?} listed twice");
        assert!(
            support.iter().all(|m| masters.contains(m)),
            "{target:?}: {support:?}"
        );
        if masters.contains(&target) {
            assert_eq!(support, vec![target.clone()]);
        }
    }
    assert!(targets.len() >= 2, "{text}");
    assert_eq!(
        targets.len() as u64,
        report["modular"]["demand"].as_u64().unwrap()
    );
}

/// `two_pass` (three-loop NO, mass z on line 0, a dotted target): pass 2
/// seeds the derivative closure up front in one rebuilt batch; the masters
/// equal the incremental closure's, and every rule both runs produce is
/// identical (same basis, unique reduced rows). With `two_pass_variants`
/// (rule ii') the basis can only shrink and the closure still closes.
#[test]
fn two_pass_equals_the_incremental_derivative_closure() {
    let base = |dir: &Path| {
        json!({
            "schema": "rustred.sector-box-reduce-spec.v1",
            "family": format!("{FIXTURES}/p3l_no/family.toml"),
            "out_dir": dir.join("out"),
            "targets": [[1, 1, 1, 1, 1, 1, 1, 1, 0], [1, 1, 1, 1, 1, 1, 1, 2, 0]],
            "derivative_slots": [0],
            "d_list": ["37/11"],
            "free": "z",
            "router": "identity",
            "lp_gate": false,
            "seeding": {"kind": "staircase", "s_pad": 0, "d_pad": 0, "max_growth": 3},
            "master_check": "report",
            "workers": 2,
        })
    };
    let dir = scratch("two-pass-ref");
    let (code, report) = run(&dir, &base(&dir));
    assert_eq!(code, 0, "{report}");
    let (masters, rules) = rules_of(&dir.join("out").join("solution-37_11.json"));
    let tdir = scratch("two-pass");
    let mut spec = base(&tdir);
    spec["two_pass"] = json!(true);
    let (code, treport) = run(&tdir, &spec);
    assert_eq!(code, 0, "{treport}");
    assert!(
        treport["modular"]["two_pass"]["masters"].as_u64().unwrap() > 0,
        "{treport}"
    );
    let (tmasters, trules) = rules_of(&tdir.join("out").join("solution-37_11.json"));
    let sorted = |mut m: Vec<Vec<i64>>| {
        m.sort();
        m
    };
    assert_eq!(sorted(masters), sorted(tmasters));
    let mut common = 0;
    for (target, terms) in &rules {
        if let Some(other) = trules.get(target) {
            assert_eq!(terms, other, "rule for {target:?} differs");
            common += 1;
        }
    }
    assert!(common >= 2, "only {common} common rules");
    // With the (ii') variants the basis can only shrink, and the closure closes.
    let vdir = scratch("two-pass-variants");
    let mut spec = base(&vdir);
    spec["two_pass"] = json!(true);
    spec["two_pass_variants"] = json!(true);
    let (code, vreport) = run(&vdir, &spec);
    assert_eq!(code, 0, "{vreport}");
    let (vmasters, _) = rules_of(&vdir.join("out").join("solution-37_11.json"));
    assert!(
        vmasters.len() <= trules_masters_len(&tdir),
        "{} masters with variants",
        vmasters.len()
    );
    assert_eq!(
        vreport["modular"]["closure"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["added"],
        0
    );
}

fn trules_masters_len(dir: &Path) -> usize {
    rules_of(&dir.join("out").join("solution-37_11.json"))
        .0
        .len()
}

/// Two-loop massless propagator family (kite, one external momentum q with
/// q.q = -1, arity 5) with mass z on the rung: the derivative closure in z
/// closes with the six masters of the one-mass kite family and both
/// certificates hold.
#[test]
fn two_loop_one_mass_kite_propagator_family_closes() {
    let dir = scratch("p2l-kite");
    let spec = json!({
        "schema": "rustred.sector-box-reduce-spec.v1",
        "family": format!("{FIXTURES}/p2l_kite/family.toml"),
        "out_dir": dir.join("out"),
        "targets": [[1, 1, 1, 1, 1]],
        "derivative_slot": 1,
        "d_list": ["37/11"],
        "free": "z",
        "seeding": {"kind": "staircase", "s_pad": 0, "d_pad": 0, "max_growth": 3},
        "lp_gate": false,
        "exact_certificate": true,
        "workers": 1,
    });
    let (code, report) = run(&dir, &spec);
    assert_eq!(code, 0, "{report}");
    assert_eq!(
        report["modular"]["masters"].as_array().unwrap().len(),
        6,
        "{report}"
    );
    let closure = report["modular"]["closure"].as_array().unwrap();
    assert_eq!(closure.last().unwrap()["added"], 0);
}

/// One mass on several lines (`derivative_slots`): the all-mass kite
/// (mass z on all five lines) closes under d/dz = sum_s a_s e_s with eight
/// masters; listing the slots in another order gives the same report masters.
#[test]
fn derivative_slots_close_the_all_mass_kite() {
    let spec = |dir: &Path, slots: Value| {
        json!({
            "schema": "rustred.sector-box-reduce-spec.v1",
            "family": format!("{FIXTURES}/p2l_kite/family-allmass.toml"),
            "out_dir": dir.join("out"),
            "targets": [[1, 1, 1, 1, 1]],
            "derivative_slots": slots,
            "d_list": ["37/11"],
            "free": "z",
            "lp_gate": false,
            "workers": 1,
        })
    };
    let dir = scratch("p2l-allmass");
    let (code, report) = run(&dir, &spec(&dir, json!([0, 1, 2, 3, 4])));
    assert_eq!(code, 0, "{report}");
    assert_eq!(
        report["modular"]["masters"].as_array().unwrap().len(),
        8,
        "{report}"
    );
    assert_eq!(
        report["modular"]["closure"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["added"],
        0
    );
    let dir2 = scratch("p2l-allmass-perm");
    let (code2, report2) = run(&dir2, &spec(&dir2, json!([4, 2, 0, 3, 1])));
    assert_eq!(code2, 0);
    assert_eq!(report["modular"]["masters"], report2["modular"]["masters"]);
}

/// `derivative_slots` adds nothing to the checkpoint key when it is empty, so
/// specs without it keep the key they had before the field existed and
/// existing checkpoints are still found; a non-empty field changes the key.
///
/// The key is a hash of the serialised spec (without paths and the
/// finite-field fields) and of the family block, which includes the router
/// fingerprint. The test therefore checks the serialisation itself, and
/// compares keys only with each other, so it holds for any router
/// fingerprint format. (For the record: with the v1 census-router
/// fingerprint, i.e. before the v2 router fingerprint, the K6 spec below,
/// modular phase only, has the key
/// d6a5ee80df8daa08481000be9c3a4b4a191c6c39ef8514c0bea8face09fc131e, the same
/// with and without this change, measured with the pre-field binary.)
#[test]
fn derivative_slots_adds_nothing_to_the_checkpoint_key_when_empty() {
    let serialised = |spec: &Value| -> String {
        let spec: rustred_app::SectorBoxReduceSpec = serde_json::from_value(spec.clone()).unwrap();
        serde_json::to_string(&serde_json::to_value(&spec).unwrap()).unwrap()
    };
    let dir = scratch("key-input");
    let mut without = k6_spec(&dir);
    without["modular_only"] = json!(true);
    let mut empty = without.clone();
    empty["derivative_slots"] = json!([]);
    let mut with_slots = without.clone();
    with_slots["derivative_slots"] = json!([0]);
    // The key input: no entry when empty, byte-identical to no field at all.
    assert!(!serialised(&without).contains("derivative_slots"));
    assert_eq!(serialised(&empty), serialised(&without));
    assert!(serialised(&with_slots).contains("\"derivative_slots\":[0]"));
    // A non-empty field changes the key (keys compared with each other only).
    let binding = |dir: &Path, spec: &Value| -> String {
        let mut spec = spec.clone();
        spec["out_dir"] = json!(dir.join("out"));
        spec["checkpoint_dir"] = json!(dir.join("ckpt"));
        let (code, report) = run(dir, &spec);
        assert_eq!(code, 0, "{report}");
        let saved: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("ckpt").join("modular.json")).unwrap(),
        )
        .unwrap();
        saved["binding"].as_str().unwrap().to_string()
    };
    let plain = binding(&dir, &without);
    let other = scratch("key-input-slots");
    assert_ne!(binding(&other, &with_slots), plain);
    let again = scratch("key-input-empty");
    assert_eq!(binding(&again, &empty), plain);
    for path in [dir, other, again] {
        std::fs::remove_dir_all(path).unwrap();
    }
}

/// Masters and rules (target -> {integral -> coefficient string}) of a solution file.
/// Rules of a solution file: target -> {integral -> coefficient string}.
type Rules = BTreeMap<Vec<i64>, BTreeMap<Vec<i64>, String>>;

fn rules_of(path: &Path) -> (Vec<Vec<i64>>, Rules) {
    let solution: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let masters: Vec<Vec<i64>> = serde_json::from_value(solution["masters"].clone()).unwrap();
    let mut rules = BTreeMap::new();
    for rule in solution["rules"].as_array().unwrap() {
        let target: Vec<i64> = serde_json::from_value(rule["target"].clone()).unwrap();
        let mut terms = BTreeMap::new();
        for term in rule["terms"].as_array().unwrap() {
            let integral: Vec<i64> = serde_json::from_value(term["integral"].clone()).unwrap();
            terms.insert(integral, term["coefficient"].as_str().unwrap().to_string());
        }
        rules.insert(target, terms);
    }
    (masters, rules)
}

fn contains_cut(integral: &[i64], cut: &[usize]) -> bool {
    cut.iter().all(|&slot| integral[slot] > 0)
}

/// Three-loop non-planar propagator family (NO) with mass z on line 0 (arity
/// 9, one external momentum): a reduction on each spanning cut through the
/// massive line (`cut_lines`, sectors without the cut lines are zero) equals
/// the uncut reduction restricted to the cut sectors, rule by rule, with the
/// uncut masters as the common preferred basis.
#[test]
fn cut_reductions_equal_the_uncut_reduction_on_the_cut_sectors() {
    let base = |dir: &Path| {
        json!({
            "schema": "rustred.sector-box-reduce-spec.v1",
            "family": format!("{FIXTURES}/p3l_no/family.toml"),
            "out_dir": dir.join("out"),
            "targets": [[1, 1, 1, 1, 1, 1, 1, 1, 0]],
            "derivative_slots": [0],
            "d_list": ["37/11"],
            "free": "z",
            "router": "identity",
            "lp_gate": true,
            "seeding": {"kind": "staircase", "s_pad": 0, "d_pad": 0, "max_growth": 3},
            "master_check": "report",
            "workers": 2,
        })
    };
    let dir = scratch("cut-uncut");
    let (code, report) = run(&dir, &base(&dir));
    assert_eq!(code, 0, "{report}");
    let (masters, uncut) = rules_of(&dir.join("out").join("solution-37_11.json"));
    for cut in [vec![0usize, 4], vec![0, 1, 6], vec![0, 1, 3, 7]] {
        let cdir = scratch(&format!(
            "cut-{}",
            cut.iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join("_")
        ));
        let mut spec = base(&cdir);
        spec["cut_lines"] = json!(cut);
        spec["preferred"] = json!(
            masters
                .iter()
                .filter(|m| contains_cut(m, &cut))
                .collect::<Vec<_>>()
        );
        let (code, report) = run(&cdir, &spec);
        assert_eq!(code, 0, "cut {cut:?}: {report}");
        let (cut_masters, cut_rules) = rules_of(&cdir.join("out").join("solution-37_11.json"));
        let expected: Vec<Vec<i64>> = masters
            .iter()
            .filter(|m| contains_cut(m, &cut))
            .cloned()
            .collect();
        let mut got = cut_masters.clone();
        got.sort();
        let mut want = expected.clone();
        want.sort();
        assert_eq!(
            got, want,
            "cut {cut:?}: masters differ from the uncut masters of the cut sectors"
        );
        assert!(!cut_rules.is_empty(), "cut {cut:?}: no rules");
        for (target, terms) in &cut_rules {
            assert!(contains_cut(target, &cut));
            let full = uncut.get(target).unwrap_or_else(|| {
                panic!("cut {cut:?}: target {target:?} not in the uncut solution")
            });
            let restricted: BTreeMap<Vec<i64>, String> = full
                .iter()
                .filter(|(integral, _)| contains_cut(integral, &cut))
                .map(|(integral, coefficient)| (integral.clone(), coefficient.clone()))
                .collect();
            assert_eq!(
                terms, &restricted,
                "cut {cut:?}: rule for {target:?} differs"
            );
        }
    }
}

/// Cluster quotients (generator-list router, three-loop NO, mass z on line 0):
/// a union of two up-closures, `cut_generators = [[0, 4], [0, 1, 6]]`, equals
/// the uncut reduction restricted to the sectors that contain at least one
/// generator, rule by rule; and a single generator reproduces the
/// `cut_lines` masters and rules exactly.
#[test]
fn generator_quotients_equal_the_uncut_reduction_on_their_sectors() {
    let base = |dir: &Path| {
        json!({
            "schema": "rustred.sector-box-reduce-spec.v1",
            "family": format!("{FIXTURES}/p3l_no/family.toml"),
            "out_dir": dir.join("out"),
            "targets": [[1, 1, 1, 1, 1, 1, 1, 1, 0]],
            "derivative_slots": [0],
            "d_list": ["37/11"],
            "free": "z",
            "router": "identity",
            "lp_gate": true,
            "seeding": {"kind": "staircase", "s_pad": 0, "d_pad": 0, "max_growth": 3},
            "master_check": "report",
            "workers": 2,
        })
    };
    let dir = scratch("gen-uncut");
    let (code, report) = run(&dir, &base(&dir));
    assert_eq!(code, 0, "{report}");
    let (masters, uncut) = rules_of(&dir.join("out").join("solution-37_11.json"));
    let generators = vec![vec![0usize, 4], vec![0, 1, 6]];
    let keeps = |integral: &[i64]| generators.iter().any(|g| contains_cut(integral, g));
    let gdir = scratch("gen-union");
    let mut spec = base(&gdir);
    spec["cut_generators"] = json!(generators);
    spec["preferred"] = json!(masters.iter().filter(|m| keeps(m)).collect::<Vec<_>>());
    let (code, report) = run(&gdir, &spec);
    assert_eq!(code, 0, "{report}");
    let (gen_masters, gen_rules) = rules_of(&gdir.join("out").join("solution-37_11.json"));
    let mut got = gen_masters.clone();
    got.sort();
    let mut want: Vec<Vec<i64>> = masters.iter().filter(|m| keeps(m)).cloned().collect();
    want.sort();
    assert_eq!(
        got, want,
        "masters differ from the uncut masters of the quotient sectors"
    );
    assert!(!gen_rules.is_empty());
    for (target, terms) in &gen_rules {
        assert!(keeps(target));
        let full = uncut
            .get(target)
            .unwrap_or_else(|| panic!("target {target:?} not in the uncut solution"));
        let restricted: BTreeMap<Vec<i64>, String> = full
            .iter()
            .filter(|(integral, _)| keeps(integral))
            .map(|(integral, coefficient)| (integral.clone(), coefficient.clone()))
            .collect();
        assert_eq!(terms, &restricted, "rule for {target:?} differs");
    }
    // one generator == cut_lines, byte for byte
    let one = vec![0usize, 1, 6];
    let adir = scratch("gen-one-a");
    let mut a = base(&adir);
    a["cut_lines"] = json!(one);
    let bdir = scratch("gen-one-b");
    let mut b = base(&bdir);
    b["cut_generators"] = json!([one]);
    assert_eq!(run(&adir, &a).0, 0);
    assert_eq!(run(&bdir, &b).0, 0);
    let sa = rules_of(&adir.join("out").join("solution-37_11.json"));
    let sb = rules_of(&bdir.join("out").join("solution-37_11.json"));
    assert_eq!(
        sa, sb,
        "a single generator must reproduce the cut_lines masters and rules"
    );
    // exclusive fields
    let cdir = scratch("gen-both");
    let mut c = base(&cdir);
    c["cut_lines"] = json!([0, 4]);
    c["cut_generators"] = json!([[0, 4]]);
    assert_ne!(
        run(&cdir, &c).0,
        0,
        "cut_lines and cut_generators together must be rejected"
    );
}

/// Spec fields added after the first spec schema, at their default value, are
/// not serialised: specs without them keep their checkpoint key.
const UNSET_FIELDS: &[&str] = &[
    "cut_lines",
    "cut_generators",
    "rebuild_rounds",
    "report_supports",
    "two_pass",
    "two_pass_variants",
    "growth_no_inherit",
    "closure_no_own_box",
    "growth_budget_seeds",
    "initial_growth",
    "resume_state",
    "round_state_dir",
];

#[test]
fn unset_fields_add_nothing_to_the_key_input() {
    let dir = scratch("key-input-unset");
    let mut spec = k6_spec(&dir);
    spec["modular_only"] = json!(true);
    let parsed: rustred_app::SectorBoxReduceSpec = serde_json::from_value(spec).unwrap();
    let serialised = serde_json::to_string(&serde_json::to_value(&parsed).unwrap()).unwrap();
    for field in UNSET_FIELDS {
        assert!(
            !serialised.contains(&format!("\"{field}\"")),
            "{field} is in the key input"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

/// `closure_no_own_box`: the derivative closure of a 3L one-mass propagator
/// family with staircase growth closes with fewer (or as many) seeds. On this
/// family the masters and the rules are the same as without the option; in
/// general the option can change the master set (non-minimal, equivalent
/// system), which this test does not cover.
#[test]
fn closure_without_own_boxes_closes_with_fewer_seeds() {
    let base = |dir: &Path| {
        json!({
            "schema": "rustred.sector-box-reduce-spec.v1",
            "family": format!("{FIXTURES}/p3l_no/family.toml"),
            "out_dir": dir.join("out"),
            "targets": [[1, 1, 1, 1, 1, 1, 1, 1, 0], [2, 1, 1, 1, 1, 1, 1, 1, 0]],
            "derivative_slots": [0],
            "d_list": ["37/11"],
            "free": "z",
            "router": "identity",
            "lp_gate": false,
            "rebuild_rounds": true,
            "growth_no_inherit": true,
            "seeding": {"kind": "staircase", "s_pad": 0, "d_pad": 0, "max_growth": 3},
            "master_check": "report",
            "workers": 2,
        })
    };
    let dir = scratch("closure-own-box-ref");
    let (code, report) = run(&dir, &base(&dir));
    assert_eq!(code, 0, "{report}");
    let ddir = scratch("closure-no-own-box");
    let mut spec = base(&ddir);
    spec["closure_no_own_box"] = json!(true);
    let (code, dreport) = run(&ddir, &spec);
    assert_eq!(code, 0, "{dreport}");
    assert_eq!(
        dreport["modular"]["closure"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["added"],
        0
    );
    let seeds = |r: &Value| r["modular"]["seeds"].as_u64().unwrap();
    assert!(
        seeds(&dreport) <= seeds(&report),
        "{} > {} seeds",
        seeds(&dreport),
        seeds(&report)
    );
    let (masters, rules) = rules_of(&dir.join("out").join("solution-37_11.json"));
    let (dmasters, drules) = rules_of(&ddir.join("out").join("solution-37_11.json"));
    let sorted = |mut m: Vec<Vec<i64>>| {
        m.sort();
        m
    };
    assert_eq!(sorted(masters), sorted(dmasters));
    for (target, terms) in &rules {
        if let Some(other) = drules.get(target) {
            assert_eq!(terms, other, "rule for {target:?} differs");
        }
    }
    for path in [dir, ddir] {
        std::fs::remove_dir_all(path).unwrap();
    }
}
