use super::*;
use crate::foundry::artifact::{ArtifactPersistenceError, derive_one_loop_unit_mass_tadpole};
use crate::sector::InteriorBounds;

#[test]
fn shared_entry_admission_counts_dots_and_numerators_not_a_coordinate_box() {
    let artifact =
        super::super::source_port::generated_scoped_k3_for_codec_test([true, true, false]);
    let rectangles = artifact.supported_root_power_bounds().to_vec();
    let sector = Mask::try_new([true, true, false]).unwrap();
    let scoped = artifact
        .with_total_excess_scope_for_test(3, BTreeMap::from([(sector.clone(), 11)]))
        .unwrap();
    assert_eq!(scoped.supported_root_power_bounds(), rectangles);
    assert!(scoped.validate_root_powers(&[3, 1, -1]).is_ok());
    assert!(scoped.validate_root_powers(&[-1, 2, -1]).is_ok());
    assert_eq!(
        scoped.validate_root_powers(&[3, 1, -2]),
        Err(RootDomainError::OutsideTotalExcess { maximum: 3 })
    );
    assert_eq!(
        scoped.validate_root_powers(&[99, 99]),
        Err(RootDomainError::WrongArity {
            expected: 3,
            actual: 2
        })
    );
    assert_eq!(
        scoped.validate_root_powers(&[99, 1, 1]),
        Err(RootDomainError::OutsideBounds {
            position: 2,
            value: 1,
            lower: i64::MIN,
            upper: 0,
        })
    );
    let ArtifactProofScope::TotalExcess(scope) = &scoped.proof_scope else {
        panic!("missing test scope")
    };
    assert_eq!(
        scope.entry.family_fingerprint(),
        scoped.family_fingerprint()
    );
    assert_eq!(scope.entry.root().active_bits(), [true, true, false]);
    assert_eq!(scope.entry.bound().limit(), 3);
    assert_eq!(scope.successor_degrees[&sector], 11);
}

#[test]
fn total_excess_entry_handles_zero_degree_and_extreme_integer_sums() {
    let artifact = derive_one_loop_unit_mass_tadpole()
        .unwrap()
        .with_total_excess_scope_for_test(0, BTreeMap::from([(Mask::try_new([true]).unwrap(), 4)]))
        .unwrap();
    for powers in [[0], [1]] {
        assert!(artifact.validate_root_powers(&powers).is_ok());
    }
    for powers in [[-1], [2], [i64::MIN], [i64::MAX]] {
        assert_eq!(
            artifact.validate_root_powers(&powers),
            Err(RootDomainError::OutsideTotalExcess { maximum: 0 })
        );
    }
    // Exercise the same owner predicate at arbitrary arity without generating
    // additional rules. This is admission arithmetic, not a closure fixture.
    let family = super::super::two_loop::canonical_family(Default::default()).unwrap();
    let scope = ArtifactProofScope::TotalExcess(TotalExcessProofScope {
        entry: EntryScope::try_new(
            &family,
            &Mask::try_new([true; 3]).unwrap(),
            super::super::source_port::scope::EntryDegreeBound::MaxTotalExcessDegree(u64::MAX),
        )
        .unwrap(),
        successor_degrees: BTreeMap::new(),
    });
    assert!(scope.admit_entry(&[i64::MIN, i64::MAX, 1]).is_ok());
    assert_eq!(
        scope.admit_entry(&[i64::MIN, i64::MIN, 0]),
        Err(RootDomainError::OutsideTotalExcess { maximum: u64::MAX })
    );
}

#[test]
fn test_scope_checks_map_arity_root_and_degree_without_granting_publication() {
    for (mask, degree) in [(vec![true, false], 3), (vec![true], 1)] {
        assert!(
            derive_one_loop_unit_mass_tadpole()
                .unwrap()
                .with_total_excess_scope_for_test(
                    2,
                    BTreeMap::from([(Mask::try_new(mask).unwrap(), degree)])
                )
                .is_err()
        );
    }
    assert!(
        derive_one_loop_unit_mass_tadpole()
            .unwrap()
            .with_total_excess_scope_for_test(2, BTreeMap::new())
            .is_err()
    );
    assert!(matches!(
        derive_one_loop_unit_mass_tadpole()
            .unwrap()
            .with_total_excess_scope_for_test(
                2,
                BTreeMap::from([(Mask::try_new([false]).unwrap(), 3)])
            ),
        Err(ArtifactError::InvalidZeroTerminal)
    ));
    let mut artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    // Admission-only negative control: the helper must reject an activating
    // successor even if a test mutates its root rectangle first.
    artifact.supported_root_power_bounds[0] = InteriorBounds::new(i64::MIN, 0);
    assert!(
        artifact
            .with_total_excess_scope_for_test(
                2,
                BTreeMap::from([(Mask::try_new([true]).unwrap(), 3)])
            )
            .is_err()
    );
}

#[test]
fn scoped_owner_cannot_claim_unrestricted_capability_or_drop_scope_on_encoding() {
    let artifact = super::super::source_port::installed_k1_for_codec_test();
    assert!(artifact.is_complete_unit_mass_vacuum());
    let bytes = artifact.encode_durable().unwrap();
    let cold = ClosedArtifact::decode_durable(&bytes).unwrap();
    assert!(cold.proof_scope.is_unrestricted());
    assert!(cold.is_complete_unit_mass_vacuum());
    assert!(
        crate::persistence::equivalent_generated_programs(
            &bytes,
            &cold.encode_durable().unwrap(),
            Default::default(),
        )
        .unwrap()
    );
    let scoped = artifact
        .with_total_excess_scope_for_test(2, BTreeMap::from([(Mask::try_new([true]).unwrap(), 4)]))
        .unwrap();
    assert!(!scoped.is_complete_unit_mass_vacuum());
    assert!(matches!(
        scoped.encode_durable(),
        Err(ArtifactPersistenceError::UnsupportedFeature {
            detail: "bounded artifact scope has no durable encoding yet"
        })
    ));
}
