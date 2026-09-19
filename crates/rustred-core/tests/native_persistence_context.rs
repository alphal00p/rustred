//! Independent integration audit of native coefficient state portability.
//!
//! Child processes intentionally have different Symbolica registration histories.
//! No process-global state reset is used, and no family solver is involved.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use rustred::algebra::{
    Coefficient, CoefficientContext, ExactAlgebraLimits, IndexedCoefficientContext,
};
use rustred::persistence::{
    BinaryIoLimits, BinaryProgramKind, BinarySection, CoefficientId, CoefficientTableBuilder,
    DecodedCoefficientTable, SectionTag, encode_program, inspect_program,
};
use symbolica::atom::{Atom, NamespacedSymbol, SymbolAttribute, SymbolBuilder};
use symbolica::coefficient::Coefficient as NativeCoefficient;

const CHILD_MODE: &str = "RUSTRED_NATIVE_CONTEXT_TEST_MODE";
const CHILD_FILE: &str = "RUSTRED_NATIVE_CONTEXT_TEST_FILE";

fn fixture() -> Vec<Coefficient> {
    let limits = ExactAlgebraLimits::default();
    let base = CoefficientContext::try_new(["native_io_d", "native_io_m2"]).unwrap();
    let d = base.parameter("native_io_d").unwrap();
    let mass = base.parameter("native_io_m2").unwrap();
    let numerator = base.try_add(&d, &mass, limits).unwrap();
    let denominator = base.try_sub(&d, &base.integer(7), limits).unwrap();
    let ratio = base.try_div(&numerator, &denominator, limits).unwrap();
    // Exercise genuinely multiprecision integer packing using Symbolica's own
    // arithmetic, not a test-local numeric or polynomial implementation.
    let large = base.unsigned_integer(u128::MAX);
    let large = base.try_mul(&large, &large, limits).unwrap();

    let empty = CoefficientContext::try_new(std::iter::empty::<String>()).unwrap();
    let indexed =
        IndexedCoefficientContext::try_new(&base, "native-persistence-cross-process-v1", 2)
            .unwrap();
    let n0 = indexed.index(0).unwrap();
    let n1 = indexed.index(1).unwrap();
    let lifted_d = indexed.lift(&d).unwrap();
    let indexed_numerator = indexed.add(&n0, &lifted_d).unwrap();
    let indexed_denominator = indexed.add(&n1, &lifted_d).unwrap();
    let indexed_ratio = indexed
        .div(&indexed_numerator, &indexed_denominator)
        .unwrap();

    vec![
        base.zero(),
        base.one(),
        base.integer(-19),
        ratio,
        large,
        empty.zero(),
        empty.one(),
        indexed.zero().raw().clone(),
        indexed.one().raw().clone(),
        indexed_ratio.raw().clone(),
    ]
}

fn register_different_history() {
    // Register the actual symbols in reverse order and their variable list in
    // a different order, alongside unrelated lists and indexed contexts.
    let reversed = CoefficientContext::try_new(["native_io_m2", "native_io_d"]).unwrap();
    let unrelated = CoefficientContext::try_new([
        "native_io_noise_c",
        "native_io_noise_a",
        "native_io_noise_b",
    ])
    .unwrap();
    let unrelated_indexed =
        IndexedCoefficientContext::try_new(&unrelated, "unrelated-context-before-import", 4)
            .unwrap();
    for value in [
        reversed.zero(),
        unrelated.one(),
        unrelated_indexed.index(3).unwrap().raw().clone(),
    ] {
        // Native numerical atom creation is the public operation that registers
        // each polynomial's variable list in Symbolica's resource table.
        let mut atom = Atom::new();
        atom.to_num(NativeCoefficient::RationalPolynomial(value));
        let _ = std::hint::black_box(atom);
    }
}

fn produce(path: &Path) {
    let limits = BinaryIoLimits::default();
    let values = fixture();
    let mut builder = CoefficientTableBuilder::new(limits);
    for (index, value) in values.iter().enumerate() {
        assert_eq!(builder.intern(value).unwrap().index(), index);
    }
    assert_eq!(builder.intern(&values[0]).unwrap().index(), 0);
    assert_eq!(builder.intern(&values[3]).unwrap().index(), 3);
    assert_eq!(builder.len(), values.len());
    let table = builder.finish().unwrap();
    let bytes = encode_program(
        BinaryProgramKind::Candidates,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: &table.state,
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: &table.atoms,
            },
        ],
        limits,
    )
    .unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn consume(path: &Path, dirty: bool) {
    if dirty {
        register_different_history();
    }
    let limits = BinaryIoLimits::default();
    let bytes = std::fs::read(path).unwrap();
    let envelope = inspect_program(&bytes, limits).unwrap();
    assert_eq!(envelope.kind(), BinaryProgramKind::Candidates);
    assert!(envelope.section(SectionTag::CERTIFICATE).is_none());
    let table = DecodedCoefficientTable::import_generated(
        envelope.section(SectionTag::SYMBOLICA_STATE).unwrap(),
        envelope.section(SectionTag::COEFFICIENTS).unwrap(),
        limits,
    )
    .unwrap();

    // Build expectations only AFTER import, so a clean reader cannot hide an
    // absent state remapping by creating the writer's maps before decoding.
    let expected = fixture();
    assert_eq!(table.len(), expected.len());
    for (index, expected) in expected.iter().enumerate() {
        let actual = table
            .coefficient(CoefficientId::try_from_index(index).unwrap())
            .unwrap();
        assert_eq!(actual, expected, "coefficient {index}, dirty={dirty}");
        assert_eq!(actual.numerator.variables(), expected.numerator.variables());
        assert_eq!(
            actual.denominator.variables(),
            expected.denominator.variables()
        );
        assert_eq!(actual.numerator.exponents, expected.numerator.exponents);
        assert_eq!(actual.denominator.exponents, expected.denominator.exponents);
        assert_eq!(
            actual.numerator.coefficients,
            expected.numerator.coefficients
        );
        assert_eq!(
            actual.denominator.coefficients,
            expected.denominator.coefficients
        );
    }
    assert!(
        table
            .coefficient(CoefficientId::try_from_index(expected.len()).unwrap())
            .is_err()
    );
}

fn reject_conflicting_symbol_attributes(path: &Path) {
    SymbolBuilder::new(NamespacedSymbol::try_parse("rustred::native_io_d").unwrap())
        .with_attributes(vec![SymbolAttribute::Symmetric])
        .build()
        .unwrap();
    let limits = BinaryIoLimits::default();
    let bytes = std::fs::read(path).unwrap();
    let envelope = inspect_program(&bytes, limits).unwrap();
    let error = DecodedCoefficientTable::import_generated(
        envelope.section(SectionTag::SYMBOLICA_STATE).unwrap(),
        envelope.section(SectionTag::COEFFICIENTS).unwrap(),
        limits,
    )
    .err()
    .expect("same-name incompatible attributes must not be silently renamed");
    assert!(error.to_string().contains("Symbol conflict"), "{error}");
}

fn child(mode: &str, file: &Path) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "native_context_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_MODE, mode)
        .env(CHILD_FILE, file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "native persistence child {mode} failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_coefficients_survive_distinct_fresh_process_symbol_histories() {
    let evidence_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../TMP");
    std::fs::create_dir_all(&evidence_root).unwrap();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = evidence_root.join(format!(
        "native-persistence-context-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let file = directory.join("mixed-context.rrbin");
    child("produce", &file);
    child("consume-clean", &file);
    child("consume-dirty", &file);
    child("reject-attributes", &file);
    // Remove only this test's explicitly created files/directory after success.
    // Failures leave their exact generated bytes in workspace TMP for diagnosis.
    std::fs::remove_file(file).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
#[ignore = "child entry point launched by the fresh-process portability test"]
fn native_context_child() {
    let mode = std::env::var(CHILD_MODE).expect("parent supplies child mode");
    let path = PathBuf::from(std::env::var_os(CHILD_FILE).expect("parent supplies output path"));
    match mode.as_str() {
        "produce" => produce(&path),
        "consume-clean" => consume(&path, false),
        "consume-dirty" => consume(&path, true),
        "reject-attributes" => reject_conflicting_symbol_attributes(&path),
        _ => panic!("unknown child mode {mode}"),
    }
}
