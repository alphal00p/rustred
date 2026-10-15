//! Explicit research fixture transport, not generation or a public extraction API.
use super::*;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

fn write_new(path: &Path, bytes: &[u8]) {
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

#[test]
#[ignore = "explicit existing-bundle extraction; requires workspace input/output environment"]
fn extract_saved_owner_fixtures_without_generation() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap();
    let input = PathBuf::from(
        std::env::var_os("RUSTRED_OWNER_FIXTURE_INPUT").expect("input path required"),
    )
    .canonicalize()
    .unwrap();
    let output = PathBuf::from(
        std::env::var_os("RUSTRED_OWNER_FIXTURE_OUTPUT").expect("fresh output path required"),
    );
    assert!(input.starts_with(&workspace));
    assert!(output.is_absolute() && !output.exists());
    assert!(
        output
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(workspace.join("TMP"))
    );
    let mut limits = CandidateBundleLimits::default();
    if let Some(entries) = std::env::var_os("RUSTRED_OWNER_FIXTURE_MAX_ENTRIES") {
        limits.max_collection_entries = entries.to_str().unwrap().parse().unwrap();
        assert!(limits.max_collection_entries > 0);
    }
    if let Some(bytes) = std::env::var_os("RUSTRED_OWNER_FIXTURE_MAX_COEFFICIENT_BYTES") {
        limits.max_total_coefficient_bytes = bytes.to_str().unwrap().parse().unwrap();
        assert!(limits.max_total_coefficient_bytes > 0);
    }
    let file = std::fs::File::open(&input).unwrap();
    let mut reader: Box<dyn Read> = if input.extension().is_some_and(|ext| ext == "gz") {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limits.bundle_byte_limit() as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= limits.bundle_byte_limit());
    let source_digest = blake3::hash(&bytes).to_hex().to_string();
    let bundle = codec::read(&bytes, limits).unwrap();
    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            limits.family_limits(),
            limits.binary_limits(),
        )
        .unwrap();
    assert_eq!(family.fingerprint(), bundle.family_fingerprint);
    let context = rustred::identity::ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let sources = rustred::solver::SourceSystem::<16>::from_family(&family).unwrap();
    let solutions =
        codec::solutions::<16>(&bundle, &context, sources.index_variables(), limits).unwrap();
    std::fs::create_dir(&output).unwrap();
    std::fs::create_dir(output.join("owners")).unwrap();
    let mut owners = Vec::new();
    let mut total_rules = 0;
    let mut total_bytes = 0;
    for (ordinal, (sector, solution)) in solutions.iter().enumerate() {
        let mut table = CoefficientTableBuilder::new(limits.binary_limits());
        let record = codec::physical_sector_record(
            family.denominator_count(),
            *sector,
            solution,
            &mut table,
        )
        .unwrap();
        let native_family =
            rustred::persistence::NativeFamilyRecord::from_family(&family, &mut table).unwrap();
        let mut program = bundle.records.clone();
        program.sectors = vec![record];
        program
            .rule_dispatch
            .retain(|entry| entry.sector == ordinal);
        for entry in &mut program.rule_dispatch {
            entry.sector = 0;
        }
        program.schema = if program.rule_dispatch.is_empty() {
            CANDIDATE_BUNDLE_SCHEMA
        } else {
            DISPATCH_CANDIDATE_BUNDLE_SCHEMA
        }
        .into();
        let encoded =
            codec::write_records(&program, &native_family, &table.finish().unwrap(), limits)
                .unwrap();
        // Fresh native import validates all coefficient/source IDs and exact
        // formulas. This does not claim original-source replay or closure.
        let restored = codec::read(&encoded, limits).unwrap();
        let restored_family = restored
            .family
            .to_family(
                &restored.coefficients,
                limits.family_limits(),
                limits.binary_limits(),
            )
            .unwrap();
        assert_eq!(restored_family.fingerprint(), family.fingerprint());
        let decoded =
            codec::solutions::<16>(&restored, &context, sources.index_variables(), limits).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].0, *sector);
        let after = &decoded[0].1;
        assert_eq!(after.finite_residuals, solution.finite_residuals);
        assert_eq!(after.rules.len(), solution.rules.len());
        assert_eq!(after.max_numerator_rank, solution.max_numerator_rank);
        assert_eq!(after.finite_case_policy, solution.finite_case_policy);
        for (left, right) in solution.rules.iter().zip(&after.rules) {
            assert_eq!(left.dispatch_policy, right.dispatch_policy);
            assert_eq!(left.candidate.case, right.candidate.case);
            assert_eq!(left.candidate.target, right.candidate.target);
            assert_eq!(left.candidate.sources, right.candidate.sources);
            assert_eq!(left.exceptions.branches, right.exceptions.branches);
            assert_eq!(left.candidate.rhs.len(), right.candidate.rhs.len());
            for (a, b) in left.candidate.rhs.iter().zip(&right.candidate.rhs) {
                assert_eq!(a.integral, b.integral);
                assert_eq!(a.coefficient, b.coefficient);
            }
        }
        let mask: String = sector[..family.denominator_count()]
            .iter()
            .map(|&on| if on { '1' } else { '0' })
            .collect();
        let relative = format!("owners/{ordinal:04}-{mask}.rrbin");
        write_new(&output.join(&relative), &encoded);
        total_rules += solution.rules.len();
        total_bytes += encoded.len();
        owners.push(
            serde_json::json!({"mask":mask,"path":relative,"bytes":encoded.len(),
            "blake3":blake3::hash(&encoded).to_hex().to_string(),"rules":solution.rules.len()}),
        );
    }
    let selection = serde_json::json!({"family_fingerprint":family.fingerprint(),"owner_count":owners.len(),
        "load_limits":{"max_collection_entries":limits.max_collection_entries,
            "max_total_coefficient_bytes":limits.max_total_coefficient_bytes},
        "owners":owners,"initial_frontier_routes":[],"source_input":input,"source_blake3":source_digest,
        "total_bundle_bytes":total_bytes,"extracted_rules":total_rules,"generated_rules":0,
        "cold_formula_equality_checked":true,"source_replay_claim":false,"closure_claim":false,
        "control_scope":"independent family literal owner control; not historical shared-routing four-loop benchmark"});
    write_new(
        &output.join("selection.json"),
        &serde_json::to_vec_pretty(&selection).unwrap(),
    );
    println!(
        "{}",
        serde_json::json!({"output":output,"owners":solutions.len(),"rules":total_rules,"generated_rules":0,"bytes":total_bytes})
    );
}
