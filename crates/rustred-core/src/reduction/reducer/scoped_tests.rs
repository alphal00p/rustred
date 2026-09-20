//! Test-only scope metadata over the unchanged, unrestricted proved applier.
use super::*;
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::sector::Mask;

#[test]
fn entry_admission_precedes_cache_and_does_not_constrain_internal_descendants() {
    let artifact = derive_one_loop_unit_mass_tadpole()
        .unwrap()
        .with_total_excess_scope_for_test(1, BTreeMap::from([(Mask::try_new([true]).unwrap(), 5)]))
        .unwrap();
    let mut reducer = Reducer::new(&artifact).unwrap();
    let child = IntegralKey::try_new([3]).unwrap(); // E=2 > entry D=1, inside test envelope.
    let mut request = ReductionRequest::default();
    let internal = reducer
        .reduce_canonical_unit_mass(&child, &mut request)
        .unwrap();
    assert_eq!(request.pending_frame_count(), 0);
    assert_eq!(reducer.cache.get(&child), Some(&internal));
    let cache = reducer.cache.clone();
    let weight = reducer.cache_weight;
    let statistics = reducer.statistics();
    assert_eq!(
        reducer.reduce_unit_mass(&child),
        Err(ReductionError::OutsideCertifiedTotalExcessDomain { maximum: 1 })
    );
    assert_eq!(
        reducer.reduce_with_common_mass_homogeneity(&child),
        Err(ReductionError::OutsideCertifiedTotalExcessDomain { maximum: 1 })
    );
    assert_eq!(
        reducer.reduce_with_common_mass_squared(&child, &artifact.coefficient_context().integer(7)),
        Err(ReductionError::OutsideCertifiedTotalExcessDomain { maximum: 1 })
    );
    assert_eq!(reducer.statistics(), statistics);
    assert_eq!(reducer.cache, cache);
    assert_eq!(reducer.cache_weight, weight);
    assert_eq!(
        reducer.reduce_unit_mass(&IntegralKey::try_new([1, 1]).unwrap()),
        Err(ReductionError::WrongArity {
            expected: 1,
            actual: 2
        })
    );
    assert_eq!(reducer.statistics(), statistics);

    let plain = derive_one_loop_unit_mass_tadpole().unwrap();
    let mut plain_reducer = Reducer::new(&plain).unwrap();
    for powers in [[-1], [0], [1], [2]] {
        let target = IntegralKey::try_new(powers).unwrap();
        assert_eq!(
            reducer.reduce_unit_mass(&target).unwrap().terms(),
            plain_reducer.reduce_unit_mass(&target).unwrap().terms()
        );
    }
}
