use crate::family::IntegralKey;
use crate::foundry::artifact::one_loop::derive_one_loop_unit_mass_tadpole;
use crate::foundry::artifact::three_loop::K6FactorizationSupport;
use crate::foundry::artifact::two_loop::derive_two_loop_unit_mass_sunset;
use crate::reduction::Reducer;
use crate::sector::Mask;

fn synthetic_three_loop_factorization_artifact() -> crate::foundry::artifact::ClosedArtifact {
    K6FactorizationSupport::try_new()
        .unwrap()
        .into_synthetic_reducer_artifact()
}

#[test]
fn compiled_k3_times_k1_factorization_recurses_to_both_parent_terminals() {
    let artifact = synthetic_three_loop_factorization_artifact();
    let embeddings = artifact.factorization_rules()[0].master_embeddings();
    assert_eq!(embeddings.len(), 2);
    assert_eq!(
        embeddings
            .iter()
            .map(|embedding| (
                embedding.raw_parent_master().powers(),
                embedding.parent_terminal().powers(),
            ))
            .collect::<Vec<_>>(),
        [
            (&[0, 0, 1, 0, 1, 1][..], &[0, 0, 1, 0, 1, 1][..]),
            (&[0, 0, 1, 1, 1, 1][..], &[0, 0, 1, 1, 1, 1][..]),
        ]
    );

    let target = IntegralKey::try_new([0, 0, 1, 2, 2, 1]).unwrap();
    let mut parent_reducer = Reducer::new(&artifact).unwrap();
    let parent = parent_reducer.reduce_unit_mass(&target).unwrap();
    let dependency_target = IntegralKey::try_new([2, 2, 1]).unwrap();
    let dependency = Reducer::new(&derive_two_loop_unit_mass_sunset().unwrap())
        .unwrap()
        .reduce_unit_mass(&dependency_target)
        .unwrap();
    assert_eq!(parent.terms().len(), 2);
    assert_eq!(
        parent.coefficient(&IntegralKey::try_new([0, 0, 1, 0, 1, 1]).unwrap()),
        dependency.coefficient(&IntegralKey::try_new([0, 1, 1]).unwrap())
    );
    assert_eq!(
        parent.coefficient(&IntegralKey::try_new([0, 0, 1, 1, 1, 1]).unwrap()),
        dependency.coefficient(&IntegralKey::try_new([1, 1, 1]).unwrap())
    );
    assert!(parent_reducer.statistics().cache_hits() >= 1);
}

#[test]
fn compiled_k1_cubed_factorizations_close_both_spanning_tree_orbits() {
    let artifact = synthetic_three_loop_factorization_artifact();
    let tadpole = derive_one_loop_unit_mass_tadpole().unwrap();
    let mut tadpole_reducer = Reducer::new(&tadpole).unwrap();
    let master = IntegralKey::try_new([1]).unwrap();
    let factors = [2, 3, 4].map(|power| {
        tadpole_reducer
            .reduce_unit_mass(&IntegralKey::try_new([power]).unwrap())
            .unwrap()
            .coefficient(&master)
            .unwrap()
            .clone()
    });
    let context = artifact.coefficient_context();
    let expected = context
        .try_mul(&factors[0], &factors[1], Default::default())
        .and_then(|product| context.try_mul(&product, &factors[2], Default::default()))
        .unwrap();

    for (rule_ordinal, sector, target) in [
        (1, [0, 0, 1, 1, 0, 1], [0, 0, 2, 3, 0, 4]),
        (2, [0, 0, 1, 0, 1, 1], [0, 0, 2, 0, 3, 4]),
    ] {
        let factorization = &artifact.factorization_rules()[rule_ordinal];
        assert_eq!(
            factorization.application_domain().sector(),
            &Mask::try_from_indices(&sector).unwrap()
        );
        assert_eq!(factorization.master_embeddings().len(), 1);
        assert_eq!(
            factorization.master_embeddings()[0]
                .raw_parent_master()
                .powers(),
            sector
        );
        assert_eq!(
            factorization.master_embeddings()[0]
                .parent_terminal()
                .powers(),
            sector
        );

        let parent = Reducer::new(&artifact)
            .unwrap()
            .reduce_unit_mass(&IntegralKey::try_new(target).unwrap())
            .unwrap();
        assert_eq!(parent.terms().len(), 1);
        assert_eq!(
            parent.coefficient(&IntegralKey::try_new(sector).unwrap()),
            Some(&expected)
        );
    }
}
