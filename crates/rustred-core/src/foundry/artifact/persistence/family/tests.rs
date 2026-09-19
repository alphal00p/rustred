use super::super::binary::Writer;
use super::*;

#[test]
fn complete_vacuum_inputs_roundtrip_without_topology_dispatch() {
    let one = crate::foundry::artifact::derive_one_loop_unit_mass_tadpole().unwrap();
    let two = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let three = crate::foundry::artifact::three_loop::canonical_family().unwrap();
    for family in [one.family(), &two, &three] {
        let mut writer = Writer::new(Default::default());
        super::super::encode_family(&mut writer, &family).unwrap();
        let (bytes, table) = writer.finish_for_test().unwrap();
        let mut reader = Reader::with_table(&bytes, Default::default(), table).unwrap();
        let decoded = decode(
            &mut reader,
            FamilyGrammar::CompleteVacuum {
                arity: family.denominator_count(),
            },
        )
        .unwrap();
        reader.finish().unwrap();
        assert_eq!(decoded.fingerprint(), family.fingerprint());
        let mut writer = Writer::new(Default::default());
        super::super::encode_family(&mut writer, &decoded).unwrap();
        assert_eq!(writer.finish(), bytes);
    }
}

#[test]
fn complete_vacuum_structure_rejects_wrong_arity_before_coefficients() {
    let family = crate::foundry::artifact::three_loop::canonical_family().unwrap();
    let mut writer = Writer::new(Default::default());
    super::super::encode_family(&mut writer, &family).unwrap();
    let bytes = writer.finish();
    let mut reader = Reader::root(&bytes, Default::default()).unwrap();
    assert!(matches!(
        decode(&mut reader, FamilyGrammar::CompleteVacuum { arity: 3 }),
        Err(ArtifactPersistenceError::SemanticMismatch { .. }),
    ));
    let mut reader = Reader::root(&bytes, Default::default()).unwrap();
    assert!(matches!(
        decode(&mut reader, FamilyGrammar::OneLoop),
        Err(ArtifactPersistenceError::SemanticMismatch {
            field: "one-loop family structural prelude",
        }),
    ));
}

#[test]
fn complete_vacuum_structure_obeys_family_matrix_budget() {
    let family = crate::foundry::artifact::three_loop::canonical_family().unwrap();
    let mut writer = Writer::new(Default::default());
    super::super::encode_family(&mut writer, &family).unwrap();
    let bytes = writer.finish();
    let mut limits = super::super::limits::ArtifactLoadLimits::default();
    limits.family.max_matrix_entries = 35;
    let mut reader = Reader::root(&bytes, limits).unwrap();
    assert!(matches!(
        decode(&mut reader, FamilyGrammar::CompleteVacuum { arity: 6 }),
        Err(ArtifactPersistenceError::ResourceLimit {
            resource: "family matrix entries",
            requested: 36,
            limit: 35,
        }),
    ));
}
