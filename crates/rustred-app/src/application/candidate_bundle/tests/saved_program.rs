//! Input-driven, separately timed checks of trusted generated pilot outputs.
//! These adapters add no algebra or closure proof. Ordinary CI uses the small
//! in-memory control; external workloads require an explicit ignored-test run.

use std::path::PathBuf;
use std::time::Instant;

use rustred::foundry::artifact::SourcePortAudit;
use serde::Deserialize;
use serde_json::{Value, json};

use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Workload {
    #[serde(default)]
    programs: Vec<PathBuf>,
    #[serde(default)]
    equal_pairs: Vec<[PathBuf; 2]>,
}

fn workload() -> Workload {
    let path = std::env::var_os("RUSTRED_SAVED_PROGRAM_CHECKS")
        .expect("set RUSTRED_SAVED_PROGRAM_CHECKS to the explicit JSON workload");
    serde_json::from_slice(&std::fs::read(path).expect("read saved-program workload"))
        .expect("parse saved-program workload")
}

pub(super) fn replay(bytes: &[u8]) -> Value {
    let limits = CandidateBundleLimits::default();
    let bundle = codec::read(bytes, limits).expect("decode trusted candidate bundle");
    macro_rules! dispatch {
        ($($n:literal),+) => {
            match bundle.root_sector.len() {
                $($n => replay_at_arity::<$n>(bundle, limits),)+
                _ => panic!("unsupported candidate arity"),
            }
        };
    }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

fn replay_at_arity<const N: usize>(bundle: Bundle, limits: CandidateBundleLimits) -> Value {
    let started = Instant::now();
    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            limits.family_limits(),
            limits.binary_limits(),
        )
        .expect("reconstruct saved family");
    assert_eq!(family.fingerprint(), bundle.family_fingerprint);
    assert_eq!(family.denominator_count(), N);
    let prepared =
        preparation::prepare::<N>(family, &bundle.root_sector, bundle.permutation.as_deref())
            .expect("prepare original family sources and zero census");
    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .expect("original IBP generator")
        .context()
        .clone();
    let solutions = codec::solutions::<N>(
        &bundle,
        &context,
        prepared.sources.index_variables(),
        limits,
    )
    .expect("reconstruct saved solutions in original coordinates");
    assert!(
        !solutions.is_empty(),
        "empty replay workload is not acceptance"
    );
    let audit =
        SourcePortAudit::try_new_with_root_sector(&prepared.family, prepared.zeros, prepared.root)
            .expect("source audit owner");
    let mut sectors = Vec::new();
    let mut total_rules = 0;
    for (sector, solution) in solutions {
        let checked = audit
            .replay_sector_rules(sector, prepared.permutation, &solution)
            .expect("exact original-source identity and guard replay");
        assert_eq!(checked.rules.len(), solution.rules.len());
        total_rules += checked.rules.len();
        sectors.push(json!({
            "sector": sector.to_vec(), "rules": checked.rules.len(),
            "source_entries": checked.rules.iter().map(|r| r.original_source_entries).sum::<usize>(),
            "replay_seconds": checked.elapsed.as_secs_f64(),
        }));
    }
    assert!(total_rules > 0, "no rule was checked");
    json!({
        "arity": N, "rules": total_rules, "sectors": sectors,
        "preparation_and_replay_seconds": started.elapsed().as_secs_f64(),
        "identity_and_guards_only": true,
        "descent_claim": false, "closure_claim": false,
    })
}

#[test]
fn saved_program_replay_adapter_checks_real_generated_identities() {
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let report = replay(generated.bundle());
    assert_eq!(report["arity"], 1);
    assert!(report["rules"].as_u64().unwrap() > 0);
    assert_eq!(report["closure_claim"], false);
}

#[test]
fn bounded_portfolio_saved_program_is_worker_deterministic_and_replayable() {
    let mut request = FamilyCandidatesRequest::new(K3);
    request.discovery_strategy = Some(
        CandidateDiscoveryStrategy::from_json(
            r#"{
        "version":2,"sectors":{"kind":"active-first"},
        "rows":{"kind":"features","priorities":[
            {"feature":{"kind":"terms"},"descending":false},
            {"feature":{"kind":"coefficient-monomials"},"descending":false}]},
        "rule_selection":{"kind":"bounded-portfolio","version":1,
            "alternatives":[{"kind":"input-order"}],
            "limits":{"max_depth":0,"max_rows":2048,
                "max_exact_trace_rows":1024,"max_exact_trace_terms":262144},
            "quality":[{"feature":"exceptional-cases","descending":false},
                {"feature":"rhs-terms","descending":false}],
            "trigger":{"kind":"always"}}
    }"#,
        )
        .unwrap(),
    );
    request.n_cores = 1;
    let serial = family_candidates(request.clone()).unwrap();
    request.n_cores = 2;
    let parallel = family_candidates(request).unwrap();
    assert_same_program(serial.bundle(), parallel.bundle());
    for generated in [&serial, &parallel] {
        let report: toml::Value = toml::from_str(generated.to_toml()).unwrap();
        assert!(report["rule_selection"]["attempted"].as_integer().unwrap() > 0);
        let checked = replay(generated.bundle());
        assert_eq!(checked["arity"], 3);
        assert!(checked["rules"].as_u64().unwrap() > 0);
    }
    // Determinism and replay do not imply that a nonbaseline rule won. Real
    // workload measurements separately require selected_alternatives > 0.
}

#[test]
#[ignore = "explicit external trusted outputs; set RUSTRED_SAVED_PROGRAM_CHECKS"]
fn saved_programs_replay_all_original_source_identities() {
    let workload = workload();
    assert!(!workload.programs.is_empty(), "no program paths supplied");
    for path in workload.programs {
        let bytes = std::fs::read(&path).expect("read saved program");
        let report = replay(&bytes);
        println!(
            "{}",
            json!({"path": path, "blake3": blake3::hash(&bytes).to_hex().to_string(),
            "check": "original-source-replay", "report": report})
        );
    }
}

#[test]
#[ignore = "explicit external baseline pairs; set RUSTRED_SAVED_PROGRAM_CHECKS"]
fn saved_program_pairs_have_identical_decoded_content() {
    let workload = workload();
    assert!(
        !workload.equal_pairs.is_empty(),
        "no comparison pairs supplied"
    );
    for [left, right] in workload.equal_pairs {
        let left_bytes = std::fs::read(&left).unwrap();
        let right_bytes = std::fs::read(&right).unwrap();
        assert_same_program(&left_bytes, &right_bytes);
        println!(
            "{}",
            json!({"left": left, "right": right,
            "left_blake3": blake3::hash(&left_bytes).to_hex().to_string(),
            "right_blake3": blake3::hash(&right_bytes).to_hex().to_string(),
            "check": "identical-decoded-program", "passed": true,
            "source_replay_claim": false, "closure_claim": false})
        );
    }
}
