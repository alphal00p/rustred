//! Regression tests for the exact induced F5 D2/N1 wrapper.

use std::collections::BTreeSet;

use rustred_legacy_oracles::Integral;
use rustred_legacy_oracles::{
    THREE_LOOP_F5_D2N1_CANONICAL_REPRESENTATIVE_POWERS, THREE_LOOP_F5_D2N1_IBPS_PER_TARGET,
    THREE_LOOP_F5_D2N1_LABELLED_TARGET_POWERS, THREE_LOOP_F5_D2N1_NATIVE_IDENTITIES,
    THREE_LOOP_F5_D2N1_ORBITS, THREE_LOOP_F5_D2N1_STABILIZER, THREE_LOOP_F5_D2N1_TARGETS,
    ThreeLoopF5D2N1Error, ThreeLoopF5D2N1Reducer, equal_mass_three_loop_tetrahedron,
    three_loop_f5_d2n1_pipeline_config,
};

fn transform(integral: &Integral, permutation: &[usize]) -> Integral {
    Integral::new(
        permutation
            .iter()
            .map(|&position| integral.powers()[position])
            .collect::<Vec<_>>(),
    )
}

#[test]
fn exact_induced_f5_d2n1_manifest_is_complete_and_replayable() {
    assert_eq!(THREE_LOOP_F5_D2N1_TARGETS, 15);
    assert_eq!(THREE_LOOP_F5_D2N1_ORBITS, 6);
    assert_eq!(THREE_LOOP_F5_D2N1_IBPS_PER_TARGET, 9);
    assert_eq!(THREE_LOOP_F5_D2N1_NATIVE_IDENTITIES, 135);
    assert_eq!(THREE_LOOP_F5_D2N1_STABILIZER.len(), 4);
    assert!(THREE_LOOP_F5_D2N1_STABILIZER.iter().all(|permutation| {
        permutation[5] == 5
            && permutation.iter().copied().collect::<BTreeSet<_>>()
                == (0..6).collect::<BTreeSet<_>>()
    }));

    let targets = THREE_LOOP_F5_D2N1_LABELLED_TARGET_POWERS
        .into_iter()
        .map(Integral::from)
        .collect::<Vec<_>>();
    assert_eq!(targets.iter().cloned().collect::<BTreeSet<_>>().len(), 15);
    for target in &targets {
        assert_eq!(
            target.powers().iter().filter(|power| **power > 0).count(),
            5
        );
        assert_eq!(
            target.powers().iter().filter(|power| **power == -1).count(),
            1
        );
        assert_eq!(target.powers()[5], -1);
        assert_eq!(
            target
                .powers()
                .iter()
                .map(|power| (*power - 1).max(0))
                .sum::<i32>(),
            2
        );
    }

    // Independently quotient all five triple-dot and ten double-dot
    // placements by the advertised order-four fixed-mask stabilizer.
    let actual_representatives = targets
        .iter()
        .map(|target| {
            THREE_LOOP_F5_D2N1_STABILIZER
                .iter()
                .map(|permutation| transform(target, permutation))
                .min()
                .unwrap()
        })
        .collect::<BTreeSet<_>>();
    let expected_representatives = THREE_LOOP_F5_D2N1_CANONICAL_REPRESENTATIVE_POWERS
        .into_iter()
        .map(Integral::from)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_representatives, expected_representatives);

    let reducer = ThreeLoopF5D2N1Reducer::build().unwrap();
    assert_eq!(reducer.config(), three_loop_f5_d2n1_pipeline_config());
    assert_eq!(reducer.targets(), targets);
    assert_eq!(reducer.reductions().len(), 15);
    assert!(!reducer.family_fingerprint().is_empty());

    // Every labelled target and every one of its 24 proven S4 images is
    // accepted.  The result is frozen to the same five-candidate whitelist.
    let family = equal_mass_three_loop_tetrahedron().unwrap();
    assert_eq!(family.symmetries().len(), 24);
    for (target, expected) in reducer.targets().iter().zip(reducer.reductions()) {
        assert_eq!(reducer.reduce_integral(target).unwrap(), *expected);
        assert!(
            expected
                .terms()
                .keys()
                .all(|terminal| reducer.candidates().contains(terminal))
        );
        for permutation in family.symmetries() {
            let image = transform(target, permutation);
            assert_eq!(
                reducer.reduce_integral(&image).unwrap(),
                *expected,
                "S4 image {image} of {target} changed its exact reduction"
            );
        }
    }

    for rejected in [
        Integral::from([2, 1, 1, 1, 1, -1]), // D1/N1
        Integral::from([3, 1, 1, 1, 1, 0]),  // D2/N0
        Integral::from([3, 1, 1, 1, 0, -1]), // B4 rather than F5
        Integral::from([4, 1, 1, 1, 1, -1]), // D3/N1
    ] {
        assert!(matches!(
            reducer.reduce_integral(&rejected),
            Err(ThreeLoopF5D2N1Error::OutsideManifest { integral }) if integral == rejected
        ));
    }
    assert!(matches!(
        reducer.reduce_integral(&Integral::from([3, 1, 1, 1, -1])),
        Err(ThreeLoopF5D2N1Error::WrongIntegralArity { actual: 5 })
    ));

    // Construction already performs both checks.  Exercise the public audit
    // surfaces independently as protection against a stale stored wrapper.
    reducer.validate_native_target_identities().unwrap();
    reducer.replay().unwrap();
}
