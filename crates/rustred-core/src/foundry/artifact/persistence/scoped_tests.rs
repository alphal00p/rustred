use super::*;
use std::collections::BTreeMap;

#[test]
fn ordinary_artifact_codecs_ignore_count_templates_but_explicit_diagnostics_enforce_them() {
    let artifact = super::super::derive_one_loop_unit_mass_tadpole().unwrap();
    let encoding = ArtifactEncodingLimits {
        max_collection_entries: 0,
        ..Default::default()
    };
    let bytes = artifact.encode_durable_with_limits(encoding).unwrap();
    assert!(
        artifact
            .encode_durable_with_limits(encoding.with_diagnostics())
            .is_err()
    );
    let loading = ArtifactLoadLimits {
        max_collection_entries: 0,
        max_index_arity: 0,
        ..Default::default()
    };
    let loaded = ClosedArtifact::decode_durable_with_limits(&bytes, loading).unwrap();
    assert_eq!(loaded.encode_durable().unwrap(), bytes);
    assert!(
        ClosedArtifact::decode_durable_with_limits(&bytes, loading.with_diagnostics()).is_err()
    );
}

#[test]
fn stopped_artifact_decoding_preserves_the_original_resource_error_and_can_be_retried() {
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

    let bytes = super::super::derive_one_loop_unit_mass_tadpole()
        .unwrap()
        .encode_durable()
        .unwrap();
    let stopped = ResourceBudget::new(ResourceLimits {
        max_elapsed: Some(std::time::Duration::ZERO),
        ..Default::default()
    })
    .run(|| ClosedArtifact::decode_durable(&bytes))
    .unwrap_err();
    assert!(matches!(
        stopped,
        ArtifactPersistenceError::NativeTransport(crate::persistence::BinaryIoError::Resource(
            ResourceLimitError::Elapsed { .. }
        ))
    ));
    assert_eq!(
        ClosedArtifact::decode_durable(&bytes)
            .unwrap()
            .encode_durable()
            .unwrap(),
        bytes
    );
}

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
