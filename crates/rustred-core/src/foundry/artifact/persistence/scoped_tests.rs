use super::*;
use std::collections::BTreeMap;

fn scoped(artifact: ClosedArtifact) -> ClosedArtifact {
    artifact
        .with_total_excess_scope_for_test(2, BTreeMap::from([(Mask::try_new([true]).unwrap(), 5)]))
        .unwrap()
}

#[test]
fn scoped_owner_is_rejected_before_the_writer_receives_any_bytes() {
    let artifact = scoped(super::super::derive_one_loop_unit_mass_tadpole().unwrap());
    let mut writer = Writer::new(Default::default());
    writer.raw(b"existing-prefix").unwrap();
    assert!(matches!(
        encode_into_writer(&artifact, &mut writer),
        Err(ArtifactPersistenceError::UnsupportedFeature {
            detail: "bounded artifact scope requires its native envelope"
        })
    ));
    assert_eq!(writer.finish(), b"existing-prefix");
}

#[test]
fn unrestricted_parent_cannot_silently_encode_a_scoped_dependency() {
    let mut parent = super::super::derive_two_loop_unit_mass_sunset().unwrap();
    assert!(parent.proof_scope.is_unrestricted());
    let dependency = parent.dependencies.remove(0);
    parent.dependencies.insert(0, Box::new(scoped(*dependency)));
    assert!(matches!(
        parent.encode_durable(),
        Err(ArtifactPersistenceError::UnsupportedFeature {
            detail: "bounded artifact scope requires its native envelope"
        })
    ));
}
