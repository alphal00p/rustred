//! Independent native terminal-value transport audit. Child processes use
//! deliberately different Symbolica histories; no global state reset is used.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rustred::algebra::{Coefficient, CoefficientContext};
use rustred::family::IntegralKey;
use rustred::foundry::artifact::ClosedArtifact;
use rustred::persistence::{
    BinaryIoLimits, BinaryProgramKind, BinarySection, ExactTerminalCatalog,
    TerminalCatalogCoverage, encode_program, inspect_program,
};
use symbolica::atom::{
    Atom, AtomCore, AtomView, NamespacedSymbol, Symbol, SymbolAttribute, SymbolBuilder,
};
use symbolica::coefficient::Coefficient as NativeCoefficient;
use symbolica::prelude::{PolyVariable, Z};

const FAMILY: &str = "native-terminal-catalog-independent-context-fixture";
const CHILD_MODE: &str = "RUSTRED_TERMINAL_CATALOG_TEST_MODE";
const CHILD_FILE: &str = "RUSTRED_TERMINAL_CATALOG_TEST_FILE";

fn symbol(name: &str, symmetric: bool) -> Symbol {
    let builder = SymbolBuilder::new(NamespacedSymbol::try_parse(name).unwrap());
    if symmetric {
        builder
            .with_attributes(vec![SymbolAttribute::Symmetric])
            .build()
            .unwrap()
    } else {
        builder.build().unwrap()
    }
}

fn key(powers: [i64; 3]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn native_rp(value: Coefficient) -> Atom {
    let mut atom = Atom::new();
    // Preserve the coefficient variant and its map, including zero/constants.
    atom.to_num(NativeCoefficient::RationalPolynomial(value));
    atom
}

fn fixture() -> ExactTerminalCatalog {
    let left = symbol("native_catalog_left::x", false);
    let right = symbol("native_catalog_right::x", false);
    let function = symbol("native_catalog_test::f", false);
    let symmetric = symbol("native_catalog_test::s", true);
    let context = CoefficientContext::try_new(["native_catalog_d", "native_catalog_m2"]).unwrap();
    let reversed = CoefficientContext::try_new(["native_catalog_m2", "native_catalog_d"]).unwrap();
    let ratio = context
        .try_div(
            &context
                .try_add(
                    &context.parameter("native_catalog_d").unwrap(),
                    &context.one(),
                    Default::default(),
                )
                .unwrap(),
            &context.parameter("native_catalog_m2").unwrap(),
            Default::default(),
        )
        .unwrap();
    let values = BTreeMap::from([
        (key([1, 0, 0]), Atom::num(0)),
        (key([0, 1, 0]), Atom::num(-19)),
        (key([0, 0, 1]), function.call((left, right))),
        (key([1, 1, 0]), symmetric.call((left, right))),
        (key([1, 0, 1]), symmetric.call((right, left))),
        (key([2, 0, 0]), native_rp(context.zero())),
        (key([0, 2, 0]), native_rp(context.one())),
        (key([0, 0, 2]), native_rp(ratio.clone())),
        (key([2, 1, 0]), native_rp(reversed.zero())),
        (key([2, 0, 1]), function.call(native_rp(ratio))),
    ]);
    assert_eq!(values[&key([1, 1, 0])], values[&key([1, 0, 1])]);
    ExactTerminalCatalog::try_new(FAMILY, 3, TerminalCatalogCoverage::Complete, values).unwrap()
}

fn dirty_context() {
    let _ = symbol("native_catalog_test::s", true);
    let _ = symbol("native_catalog_test::f", false);
    let _ = symbol("native_catalog_right::x", false);
    let _ = symbol("native_catalog_left::x", false);
    let reversed = CoefficientContext::try_new(["native_catalog_m2", "native_catalog_d"]).unwrap();
    let noise =
        CoefficientContext::try_new(["native_catalog_noise_b", "native_catalog_noise_a"]).unwrap();
    let _ = std::hint::black_box(native_rp(reversed.zero()));
    let _ = std::hint::black_box(native_rp(noise.one()));
}

fn produce(path: &Path) {
    let bytes = fixture().encode_native(BinaryIoLimits::default()).unwrap();
    assert_eq!(
        inspect_program(&bytes, Default::default()).unwrap().kind(),
        BinaryProgramKind::TerminalValues
    );
    std::fs::write(path, bytes).unwrap();
}

fn consume(path: &Path, dirty: bool) {
    if dirty {
        dirty_context();
    }
    let bytes = std::fs::read(path).unwrap();
    // Import before creating expectations, so expectation setup cannot mask
    // missing native namespace/attribute/variable-map remapping.
    let actual =
        ExactTerminalCatalog::decode_generated(&bytes, FAMILY, 3, Default::default()).unwrap();
    let expected = fixture();
    assert_eq!(actual, expected);
    for (key, expected) in expected.terms() {
        let actual = &actual.terms()[key];
        if let (AtomView::Num(left), AtomView::Num(right)) = (actual.as_view(), expected.as_view())
        {
            if let (
                NativeCoefficient::RationalPolynomial(left),
                NativeCoefficient::RationalPolynomial(right),
            ) = (
                left.get_coeff_view().to_owned(),
                right.get_coeff_view().to_owned(),
            ) {
                assert_eq!(left.numerator.variables(), right.numerator.variables());
                assert_eq!(left.denominator.variables(), right.denominator.variables());
                assert_eq!(left, right);
            }
        }
    }
    let again = ExactTerminalCatalog::decode_generated(
        &actual.encode_native(Default::default()).unwrap(),
        FAMILY,
        3,
        Default::default(),
    )
    .unwrap();
    assert_eq!(actual, again);
    assert!(
        ExactTerminalCatalog::decode_generated(&bytes, "other-family", 3, Default::default())
            .is_err()
    );
    assert!(ExactTerminalCatalog::decode_generated(&bytes, FAMILY, 4, Default::default()).is_err());
    assert!(ClosedArtifact::decode_durable(&bytes).is_err());
    let envelope = inspect_program(&bytes, Default::default()).unwrap();
    let sections = envelope
        .sections()
        .iter()
        .map(|section| BinarySection {
            tag: section.tag,
            bytes: section.bytes,
        })
        .collect::<Vec<_>>();
    for kind in [BinaryProgramKind::Candidates, BinaryProgramKind::Certified] {
        let wrong_kind = encode_program(kind, &sections, Default::default()).unwrap();
        assert!(
            ExactTerminalCatalog::decode_generated(&wrong_kind, FAMILY, 3, Default::default())
                .is_err()
        );
    }
}

fn reject_conflicting_attributes(path: &Path) {
    let _ = symbol("native_catalog_test::s", false);
    let bytes = std::fs::read(path).unwrap();
    let error = ExactTerminalCatalog::decode_generated(&bytes, FAMILY, 3, Default::default())
        .expect_err("a conflicting same-name function must not be silently renamed");
    assert!(error.to_string().contains("Symbol conflict"), "{error}");
}

fn child(mode: &str, path: &Path) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "native_terminal_catalog_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_MODE, mode)
        .env(CHILD_FILE, path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "catalog child {mode} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn exact_catalog_values_survive_distinct_fresh_symbol_histories() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../TMP");
    std::fs::create_dir_all(&root).unwrap();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = root.join(format!(
        "native-terminal-catalog-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("exact-values.rrbin");
    child("produce", &path);
    child("consume-clean", &path);
    child("consume-dirty", &path);
    child("reject-attributes", &path);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn approximate_numbers_hidden_in_native_polynomial_variables_are_rejected() {
    let function = symbol("native_catalog_approx::f", false);
    let variable = symbol("native_catalog_approx::x", false);
    let approximate = Atom::num(0.125_f64);
    assert!(
        matches!(approximate.as_view(), AtomView::Num(number) if number.get_coeff_view().is_float())
    );
    let nested = function.call(approximate.clone());
    let power = variable.to_atom().pow(&approximate);
    let mut values = vec![approximate, nested.clone(), power.clone()];
    for variable in [
        PolyVariable::Function(function, nested),
        PolyVariable::Power(power),
    ] {
        let mut polynomial = Coefficient::new(&Z, Arc::new(vec![variable]));
        polynomial.numerator.append_monomial(1.into(), &[1]);
        values.push(native_rp(polynomial));
    }
    for (case, value) in values.into_iter().enumerate() {
        assert!(
            ExactTerminalCatalog::try_new(
                FAMILY,
                3,
                TerminalCatalogCoverage::Partial,
                BTreeMap::from([(key([1, 1, 1]), value)]),
            )
            .is_err(),
            "approximate coefficient hidden in case {case} was accepted"
        );
    }
}

#[test]
#[ignore = "fresh-process entry point driven by catalog portability test"]
fn native_terminal_catalog_child() {
    let mode = std::env::var(CHILD_MODE).expect("parent supplies child mode");
    let path = PathBuf::from(std::env::var_os(CHILD_FILE).expect("parent supplies path"));
    match mode.as_str() {
        "produce" => produce(&path),
        "consume-clean" => consume(&path, false),
        "consume-dirty" => consume(&path, true),
        "reject-attributes" => reject_conflicting_attributes(&path),
        _ => panic!("unexpected child mode {mode}"),
    }
}
