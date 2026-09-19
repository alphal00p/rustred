use super::*;
use crate::persistence::{
    BinaryProgramKind, BinarySection, SectionTag, encode_program, inspect_program,
};

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn sample() -> ExactTerminalCatalog {
    ExactTerminalCatalog::try_new(
        "generic-catalog-family",
        3,
        TerminalCatalogCoverage::Complete,
        BTreeMap::from([
            (key(&[-3, 0, 1]), Atom::Zero),
            (
                key(&[0, 1, 1]),
                symbolica::parse!("catalog_test::Gam(1,1)^2/catalog_test::ep^2"),
            ),
            (
                key(&[1, 0, 1]),
                symbolica::parse!("catalog_test::Gam(1,1)^2/catalog_test::ep^2"),
            ),
            (
                key(&[1, 1, 1]),
                symbolica::parse!("catalog_test::PR4+catalog_test::PR9/2"),
            ),
        ]),
    )
    .unwrap()
}

fn replace_section(bytes: &[u8], tag: SectionTag, replacement: &[u8]) -> Vec<u8> {
    let envelope = inspect_program(bytes, Default::default()).unwrap();
    let sections = envelope
        .sections()
        .iter()
        .map(|section| BinarySection {
            tag: section.tag,
            bytes: if section.tag == tag {
                replacement
            } else {
                section.bytes
            },
        })
        .collect::<Vec<_>>();
    encode_program(envelope.kind(), &sections, Default::default()).unwrap()
}

#[test]
fn arbitrary_exact_expressions_roundtrip_with_deduplicated_values() {
    let catalog = sample();
    let bytes = catalog.encode_native(Default::default()).unwrap();
    let envelope = inspect_program(&bytes, Default::default()).unwrap();
    assert_eq!(envelope.kind(), BinaryProgramKind::TerminalValues);
    let table = envelope.section(SectionTag::VALUES).unwrap();
    assert_eq!(u64::from_le_bytes(table[..8].try_into().unwrap()), 3);
    assert_eq!(
        ExactTerminalCatalog::decode_generated(
            &bytes,
            "generic-catalog-family",
            3,
            Default::default()
        )
        .unwrap(),
        catalog
    );
    assert_eq!(catalog.encode_native(Default::default()).unwrap(), bytes);
}

#[test]
fn full_i64_keys_and_partial_or_empty_coverage_are_preserved() {
    for coverage in [
        TerminalCatalogCoverage::Partial,
        TerminalCatalogCoverage::Complete,
    ] {
        for terms in [
            BTreeMap::new(),
            BTreeMap::from([(key(&[i64::MIN, i64::MAX]), Atom::num(-7))]),
        ] {
            let catalog =
                ExactTerminalCatalog::try_new("two-arbitrary-indices", 2, coverage, terms).unwrap();
            let bytes = catalog.encode_native(Default::default()).unwrap();
            assert_eq!(
                ExactTerminalCatalog::decode_generated(
                    &bytes,
                    "two-arbitrary-indices",
                    2,
                    Default::default()
                )
                .unwrap(),
                catalog
            );
        }
    }
}

#[test]
fn value_catalog_cannot_grant_candidate_or_certified_authority() {
    let bytes = sample().encode_native(Default::default()).unwrap();
    assert!(crate::foundry::artifact::ClosedArtifact::decode_durable(&bytes).is_err());
    let envelope = inspect_program(&bytes, Default::default()).unwrap();
    for kind in [BinaryProgramKind::Candidates, BinaryProgramKind::Certified] {
        let forged = encode_program(kind, envelope.sections(), Default::default()).unwrap();
        assert!(
            ExactTerminalCatalog::decode_generated(
                &forged,
                "generic-catalog-family",
                3,
                Default::default()
            )
            .is_err()
        );
    }
}

#[test]
fn catalog_rejects_mismatched_identity_arity_and_approximate_nested_values() {
    let bytes = sample().encode_native(Default::default()).unwrap();
    assert!(
        ExactTerminalCatalog::decode_generated(&bytes, "wrong-family", 3, Default::default())
            .is_err()
    );
    assert!(
        ExactTerminalCatalog::decode_generated(
            &bytes,
            "generic-catalog-family",
            4,
            Default::default()
        )
        .is_err()
    );
    for expression in [
        "0.125",
        "catalog_test::f(0.125)",
        "catalog_test::f(catalog_test::x)^0.125",
    ] {
        let value = symbolica::parse!(expression);
        assert!(
            ExactTerminalCatalog::try_new(
                "f",
                1,
                TerminalCatalogCoverage::Complete,
                BTreeMap::from([(key(&[1]), value)])
            )
            .is_err(),
            "{expression}"
        );
    }
    assert!(
        ExactTerminalCatalog::try_new("", 1, TerminalCatalogCoverage::Complete, BTreeMap::new())
            .is_err()
    );
    assert!(
        ExactTerminalCatalog::try_new("f", 0, TerminalCatalogCoverage::Complete, BTreeMap::new())
            .is_err()
    );
    assert!(
        ExactTerminalCatalog::try_new(
            "f",
            2,
            TerminalCatalogCoverage::Complete,
            BTreeMap::from([(key(&[1]), Atom::num(1))])
        )
        .is_err()
    );
}

#[test]
fn malformed_frames_and_restrictive_limits_fail_closed() {
    let bytes = sample().encode_native(Default::default()).unwrap();
    let decode = |bytes: &[u8]| {
        ExactTerminalCatalog::decode_generated(
            bytes,
            "generic-catalog-family",
            3,
            Default::default(),
        )
    };
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode(&trailing).is_err());
    assert!(matches!(
        ExactTerminalCatalog::decode_generated(
            &bytes,
            "generic-catalog-family",
            3,
            BinaryIoLimits {
                max_program_bytes: bytes.len() - 1,
                ..Default::default()
            }
        ),
        Err(BinaryIoError::Limit { .. })
    ));
    assert!(
        sample()
            .encode_native(BinaryIoLimits {
                max_atom_bytes: 1,
                ..Default::default()
            })
            .is_err()
    );
    assert!(
        decode(&replace_section(
            &bytes,
            SectionTag::VALUES,
            &u64::MAX.to_le_bytes()
        ))
        .is_err()
    );
    let envelope = inspect_program(&bytes, Default::default()).unwrap();
    let mut records = envelope.section(SectionTag::PROGRAM).unwrap().to_vec();
    records[0] = 2;
    assert!(decode(&replace_section(&bytes, SectionTag::PROGRAM, &records)).is_err());
    let mut frames = envelope.section(SectionTag::VALUES).unwrap().to_vec();
    frames[17] ^= 1; // first native Atom's inner length, not its algebra.
    assert!(decode(&replace_section(&bytes, SectionTag::VALUES, &frames)).is_err());
}

#[test]
fn compact_structural_keys_do_not_use_eighty_bytes_per_ten_index_key() {
    let catalog = ExactTerminalCatalog::try_new(
        "ten-indices",
        10,
        TerminalCatalogCoverage::Complete,
        (0..100)
            .map(|i| (key(&[i, 1, 1, 1, 1, 1, 1, 1, 1, 0]), Atom::num(1)))
            .collect(),
    )
    .unwrap();
    let bytes = catalog.encode_native(Default::default()).unwrap();
    let envelope = inspect_program(&bytes, Default::default()).unwrap();
    assert!(envelope.section(SectionTag::PROGRAM).unwrap().len() < 1400);
    assert_eq!(
        ExactTerminalCatalog::decode_generated(&bytes, "ten-indices", 10, Default::default())
            .unwrap(),
        catalog
    );
}
