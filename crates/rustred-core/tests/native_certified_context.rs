//! Fresh-process durability audit for native certified programs and nested owners.
//!
//! Only generated K1/K3 payloads cross the native import boundary. Consumers
//! deliberately register different symbols and polynomial maps before loading;
//! no unsafe process-global Symbolica reset is used.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rustred::algebra::{Coefficient, CoefficientContext, IndexedCoefficientContext};
use rustred::family::{AffineDenominator, IntegralFamily, IntegralKey};
use rustred::foundry::artifact::{
    ClosedArtifact, SourcePortAudit, derive_one_loop_unit_mass_tadpole,
    derive_two_loop_unit_mass_sunset,
};
use rustred::persistence::{
    BinaryIoLimits, BinaryProgramKind, equivalent_generated_programs, inspect_program,
};
use rustred::reduction::{Reducer, ReductionError};
use rustred::sector::{Mask, zero};
use rustred::solver::{SectorConfig, SectorSolveOptions, SectorSolver, SourceSystem};
use symbolica::atom::{Atom, NamespacedSymbol, SymbolBuilder};
use symbolica::coefficient::Coefficient as NativeCoefficient;
use symbolica::prelude::{PolyVariable, Z};

const CHILD_MODE: &str = "RUSTRED_CERTIFIED_CONTEXT_TEST_MODE";
const CHILD_DIRECTORY: &str = "RUSTRED_CERTIFIED_CONTEXT_TEST_DIRECTORY";

fn generated(arity: usize) -> ClosedArtifact {
    match arity {
        1 => derive_one_loop_unit_mass_tadpole().unwrap(),
        3 => derive_two_loop_unit_mass_sunset().unwrap(),
        _ => unreachable!("test fixture arity"),
    }
}

fn file(directory: &Path, arity: usize) -> PathBuf {
    directory.join(format!("k{arity}.certified.rrbin"))
}

fn bounded_file(directory: &Path, arity: usize) -> PathBuf {
    directory.join(format!("k{arity}.bounded.rrbin"))
}

fn generated_bounded(arity: usize) -> ClosedArtifact {
    // Concrete external validation fixtures; neither production strategy nor
    // fixture authority comes from a registered small-family producer.
    let (loops, columns) = match arity {
        1 => (vec!["k".into()], vec![vec![1]]),
        3 => (
            vec!["k1".into(), "k2".into()],
            vec![vec![1, 0, 0], vec![0, 0, 1], vec![1, 2, 1]],
        ),
        _ => unreachable!("test fixture arity"),
    };
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let family = IntegralFamily::new(
        &format!("bounded-portability-{arity}"),
        loops,
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        columns
            .into_iter()
            .map(|column| {
                AffineDenominator::new(
                    context.integer(-1),
                    column
                        .into_iter()
                        .map(|entry| context.integer(entry))
                        .collect(),
                )
            })
            .collect(),
        Vec::new(),
        vec![context.zero(); arity],
    )
    .unwrap();
    match arity {
        1 => solve_bounded::<1>(family),
        3 => solve_bounded::<3>(family),
        _ => unreachable!("test fixture arity"),
    }
}

fn solve_bounded<const N: usize>(family: IntegralFamily) -> ClosedArtifact {
    let analyzer = zero::Analyzer::try_unrestricted(&family).unwrap();
    let mut zeros = Vec::new();
    let mut sectors = Vec::new();
    for bits in 0..(1usize << N) {
        let sector: [bool; N] = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        match analyzer.analyze(&Mask::try_new(sector).unwrap()).unwrap() {
            zero::Decision::ProvedZero(_) => zeros.push(sector),
            zero::Decision::Inconclusive(_) => sectors.push(sector),
            zero::Decision::Excluded(_) => panic!("unrestricted fixture was excluded"),
        }
    }
    drop(analyzer);
    let zeros: Arc<[[bool; N]]> = zeros.into();
    let sources = SourceSystem::from_family(&family).unwrap();
    let solved: Vec<_> = sectors
        .into_iter()
        .map(|sector| {
            let solution = SectorSolver::new(
                &sources,
                sector,
                SectorConfig {
                    zero_sectors: zeros.clone(),
                    ..Default::default()
                },
            )
            .unwrap()
            .solve_sector(SectorSolveOptions::default())
            .unwrap();
            (sector, None, solution)
        })
        .collect();
    SourcePortAudit::try_new(&family, zeros)
        .unwrap()
        .install_complete_through_total_excess(family, solved, 2)
        .unwrap()
}

fn register_different_history() {
    // Register real private index symbols in reverse order before the actual
    // base dimension symbol, then register a different variable-list order.
    let mut reverse_indices = Vec::new();
    for axis in [2, 1, 0] {
        let name = format!("rustred_indexed_coefficient_v1::n{axis}");
        let symbol = SymbolBuilder::new(NamespacedSymbol::try_parse(&name).unwrap())
            .build()
            .unwrap();
        reverse_indices.push(PolyVariable::Symbol(symbol));
    }
    let noise =
        CoefficientContext::try_new(["certified_context_noise_b", "certified_context_noise_a"])
            .unwrap();
    let dimension = CoefficientContext::try_new(["d"]).unwrap();
    reverse_indices.extend(dimension.one().get_variables().iter().cloned());
    let reverse = Coefficient::new(&Z, Arc::new(reverse_indices));
    let unrelated =
        IndexedCoefficientContext::try_new(&noise, "unrelated-certified-import", 5).unwrap();
    for value in [reverse, noise.one(), unrelated.one().raw().clone()] {
        let mut atom = Atom::new();
        atom.to_num(NativeCoefficient::RationalPolynomial(value));
        let _ = std::hint::black_box(atom);
    }
}

fn produce(directory: &Path) {
    for arity in [1, 3] {
        let artifact = generated(arity);
        let bytes = artifact.encode_durable().unwrap();
        assert_eq!(
            inspect_program(&bytes, BinaryIoLimits::default())
                .unwrap()
                .kind(),
            BinaryProgramKind::Certified,
        );
        std::fs::write(file(directory, arity), bytes).unwrap();
    }
}

fn compare_owners(loaded: &ClosedArtifact, expected: &ClosedArtifact) {
    assert_eq!(loaded.schema(), expected.schema());
    assert_eq!(loaded.algorithm_id(), expected.algorithm_id());
    assert_eq!(loaded.arity(), expected.arity());
    assert_eq!(loaded.family_fingerprint(), expected.family_fingerprint());
    assert_eq!(loaded.context_fingerprint(), expected.context_fingerprint());
    assert_eq!(loaded.ordering(), expected.ordering());
    match (loaded.total_excess_scope(), expected.total_excess_scope()) {
        (Some(left), Some(right)) => {
            assert_eq!(left.root_sector(), right.root_sector());
            assert_eq!(
                left.max_entry_total_excess_degree(),
                right.max_entry_total_excess_degree()
            );
            assert_eq!(left.successor_degrees(), right.successor_degrees());
        }
        (None, None) => {}
        _ => panic!("proof scopes differ"),
    }
    assert_eq!(loaded.source_relations(), expected.source_relations());
    assert_eq!(loaded.rules(), expected.rules());
    assert_eq!(loaded.rule_cells().len(), expected.rule_cells().len());
    assert_eq!(loaded.masters(), expected.masters());
    assert_eq!(loaded.zero_sectors(), expected.zero_sectors());
    assert_eq!(loaded.validation(), expected.validation());
    assert_eq!(
        loaded.factorization_rules().len(),
        expected.factorization_rules().len()
    );
    assert_eq!(loaded.dependencies().len(), expected.dependencies().len());
    let left = loaded.encode_durable().unwrap();
    let right = expected.encode_durable().unwrap();
    assert!(equivalent_generated_programs(&left, &right, BinaryIoLimits::default()).unwrap());
    for (left, right) in loaded.dependencies().iter().zip(expected.dependencies()) {
        compare_owners(left, right);
    }
}

fn produce_bounded(directory: &Path) {
    for arity in [1, 3] {
        let artifact = generated_bounded(arity);
        let bytes = artifact.encode_durable().unwrap();
        assert_eq!(
            inspect_program(&bytes, Default::default()).unwrap().kind(),
            BinaryProgramKind::BoundedCertified
        );
        std::fs::write(bounded_file(directory, arity), bytes).unwrap();
    }
}

fn consume_bounded(directory: &Path, dirty: bool) {
    if dirty {
        register_different_history();
    }
    let loaded: Vec<_> = [1, 3]
        .into_iter()
        .map(|arity| {
            let bytes = std::fs::read(bounded_file(directory, arity)).unwrap();
            let artifact = ClosedArtifact::decode_durable(&bytes).unwrap();
            (arity, bytes, artifact)
        })
        .collect();
    for (arity, bytes, artifact) in loaded {
        let expected = generated_bounded(arity);
        compare_owners(&artifact, &expected);
        assert!(!artifact.is_complete_unit_mass_vacuum());
        assert!(ClosedArtifact::decode_complete_unit_mass_vacuum(&bytes).is_err());
        let scope = artifact.total_excess_scope().unwrap();
        assert_eq!(scope.max_entry_total_excess_degree(), 2);
        let targets = if arity == 1 {
            vec![vec![1], vec![3], vec![-2]]
        } else {
            assert!(scope.successor_degrees().values().any(|&degree| degree > 2));
            vec![
                vec![3, 1, 1],
                vec![2, 2, 1],
                vec![2, -1, 1],
                vec![1, -2, 1],
                vec![0, 1, 1],
            ]
        };
        let mut actual = Reducer::new(&artifact).unwrap();
        let mut reference = Reducer::new(&expected).unwrap();
        for powers in targets {
            let key = IntegralKey::try_new(powers).unwrap();
            let left = actual
                .reduce_with_common_mass_squared(&key, &artifact.coefficient_context().integer(3))
                .unwrap();
            let right = reference
                .reduce_with_common_mass_squared(&key, &expected.coefficient_context().integer(3))
                .unwrap();
            assert_eq!(left, right);
            for (key, value) in left.terms() {
                assert_eq!(
                    value.numerator.variables(),
                    right.terms()[key].numerator.variables()
                );
                assert_eq!(
                    value.denominator.variables(),
                    right.terms()[key].denominator.variables()
                );
            }
            let before = actual.statistics();
            assert_eq!(
                actual
                    .reduce_with_common_mass_squared(
                        &key,
                        &artifact.coefficient_context().integer(3)
                    )
                    .unwrap(),
                left
            );
            assert_eq!(
                actual.statistics().rule_applications(),
                before.rule_applications()
            );
        }
        let mut outside = vec![1; arity];
        outside[0] = 4;
        let before = actual.statistics();
        assert_eq!(
            actual.reduce_unit_mass(&IntegralKey::try_new(outside).unwrap()),
            Err(ReductionError::OutsideCertifiedTotalExcessDomain { maximum: 2 })
        );
        assert_eq!(actual.statistics(), before);
    }
}

fn consume(directory: &Path, dirty: bool) {
    if dirty {
        register_different_history();
    }
    // Import both inputs BEFORE deriving any expected family. The expectation
    // setup must not accidentally recreate the writer's registration history.
    let loaded: Vec<_> = [1, 3]
        .into_iter()
        .map(|arity| {
            let bytes = std::fs::read(file(directory, arity)).unwrap();
            let artifact = ClosedArtifact::decode_durable(&bytes).unwrap();
            (arity, bytes, artifact)
        })
        .collect();
    for (arity, bytes, artifact) in loaded {
        let expected = generated(arity);
        compare_owners(&artifact, &expected);
        let reencoded = artifact.encode_durable().unwrap();
        assert!(
            equivalent_generated_programs(&bytes, &reencoded, BinaryIoLimits::default()).unwrap()
        );
        let targets: Vec<Vec<i64>> = if arity == 1 {
            vec![vec![3]]
        } else {
            assert!(!artifact.rule_cells().is_empty());
            assert!(
                !artifact.dependencies().is_empty(),
                "K3 must exercise nested K1 owners"
            );
            assert!(!artifact.factorization_rules().is_empty());
            vec![vec![2, 1, 1], vec![0, 2, 3]]
        };
        let mut actual_reducer = Reducer::new(&artifact).unwrap();
        let mut expected_reducer = Reducer::new(&expected).unwrap();
        for target in targets {
            let key = IntegralKey::try_new(target).unwrap();
            let actual = actual_reducer.reduce_unit_mass(&key).unwrap();
            let expected = expected_reducer.reduce_unit_mass(&key).unwrap();
            assert_eq!(actual, expected);
            assert!(!actual.is_zero());
            assert!(
                actual
                    .terms()
                    .keys()
                    .all(|key| artifact.masters().contains(key))
            );
        }
        assert!(actual_reducer.statistics().rule_applications() > 0);
    }
}

fn child(mode: &str, directory: &Path) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "native_certified_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_MODE, mode)
        .env(CHILD_DIRECTORY, directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "certified child {mode} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn certified_k1_k3_survive_fresh_dirty_symbol_contexts() {
    let evidence = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../TMP");
    std::fs::create_dir_all(&evidence).unwrap();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = evidence.join(format!(
        "native-certified-context-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    child("produce", &directory);
    child("consume-clean", &directory);
    child("consume-dirty", &directory);
    // Remove only this test's concrete files; preserve evidence after failure.
    for arity in [1, 3] {
        std::fs::remove_file(file(&directory, arity)).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn bounded_native_k1_k3_survive_fresh_dirty_symbol_contexts() {
    let evidence = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../TMP");
    std::fs::create_dir_all(&evidence).unwrap();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = evidence.join(format!(
        "native-bounded-context-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    child("produce-bounded", &directory);
    child("consume-bounded-clean", &directory);
    child("consume-bounded-dirty", &directory);
    for arity in [1, 3] {
        std::fs::remove_file(bounded_file(&directory, arity)).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
}

#[test]
#[ignore = "fresh-process entry point driven by certified portability test"]
fn native_certified_child() {
    let mode = std::env::var(CHILD_MODE).expect("parent supplies mode");
    let directory =
        PathBuf::from(std::env::var_os(CHILD_DIRECTORY).expect("parent supplies directory"));
    match mode.as_str() {
        "produce" => produce(&directory),
        "consume-clean" => consume(&directory, false),
        "consume-dirty" => consume(&directory, true),
        "produce-bounded" => produce_bounded(&directory),
        "consume-bounded-clean" => consume_bounded(&directory, false),
        "consume-bounded-dirty" => consume_bounded(&directory, true),
        _ => panic!("unexpected child mode {mode}"),
    }
}
