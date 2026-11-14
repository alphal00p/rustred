//! `rustred sector-box-reduce`: the CLI against golden eta-derivative
//! reductions produced by the campaign driver of the sector-box engine.
//!
//! Default tests (seconds): the three-loop K6 vacuum family with one massive
//! line deformed by `eta` (2 dimension values, 12 masters), checkpoint
//! resume, the certificate-prime rule, a two-loop family at arity 3 and the
//! input-error contract. Ignored: the four-loop prism family at 68 dimension
//! values; see `prism_4l_eta_derivative_equations_match_the_campaign`.
use std::collections::BTreeMap;
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
    // solution is written, the certificate fails, exit 1.
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
    for (label, spec) in cases {
        let (code, report) = run(&dir, &spec);
        assert_eq!(code, 4, "{label}: {report}");
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
