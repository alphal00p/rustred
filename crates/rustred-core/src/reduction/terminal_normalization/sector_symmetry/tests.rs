use super::*;
use std::cell::Cell;

use crate::algebra::matrix::{SymbolicaCoefficientMatrixLimits, rank_of_coefficient_matrix};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::AffineDenominator;
use crate::family::isp::IspCompletion;
use crate::reduction::terminal_normalization::TerminalNumeratorSymmetryWitness;
use crate::sector::symmetry::CoefficientMatrix;

fn test_family(masses: [&str; 3]) -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "sector_symmetry_test",
            vec!["k".into(), "l".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
                .into_iter()
                .zip(masses)
                .map(|(row, mass)| {
                    AffineDenominator::new(
                        context
                            .try_neg(&context.coefficient_fixture(mass), Default::default())
                            .unwrap(),
                        row.map(|value| context.integer(value)).into(),
                    )
                })
                .collect(),
            vec![],
            vec![context.zero(); 3],
        )
        .unwrap(),
    )
}

fn key(powers: [i64; 3]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

fn map(family: &IntegralFamily, entries: [i64; 4]) -> MomentumMap {
    MomentumMap::new(
        CoefficientMatrix::try_new(
            2,
            2,
            entries.map(|value| family.coefficient_context().integer(value)),
        )
        .unwrap(),
        CoefficientMatrix::try_new(2, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    )
}

fn root(powers: [i64; 3]) -> Mask {
    Mask::try_from_indices(&powers).unwrap()
}

#[test]
fn unequal_dots_give_an_exact_cross_key_equation_rejected_by_the_weaker_lane() {
    let family = test_family(["1", "1", "1"]);
    let source = key([1, 2, -1]);
    let swap = map(&family, [0, 1, 1, 0]);
    assert!(
        TerminalNumeratorSymmetryWitness::verify(
            family.clone(),
            &source,
            swap.clone(),
            Default::default(),
        )
        .is_err()
    );
    let proof =
        TerminalSectorSymmetryWitness::verify(family.clone(), &source, swap, Default::default())
            .unwrap();
    let context = family.coefficient_context();
    assert_eq!(
        proof.equation().terms,
        BTreeMap::from([
            (source.clone(), context.one()),
            (key([2, 1, -1]), context.integer(-1)),
        ])
    );
    assert!(proof.equation().nonzero_conditions.is_empty());
    assert_eq!(proof.source(), &source);
    assert_eq!(proof.family_fingerprint(), family.fingerprint());
    proof.replay(family, Default::default()).unwrap();
}

#[test]
fn reflection_retains_the_actual_rational_mass_constants() {
    let family = test_family(["3/2", "5/3", "7/4"]);
    let source = key([1, 1, -1]);
    let proof = TerminalSectorSymmetryWitness::verify(
        family.clone(),
        &source,
        map(&family, [-1, 0, 0, 1]),
        Default::default(),
    )
    .unwrap();
    let context = family.coefficient_context();
    // D2 -> 2 D0 + 2 D1 - D2 + 2(3/2 + 5/3 - 7/4).
    assert_eq!(
        proof.equation().terms,
        BTreeMap::from([
            (source, context.integer(2)),
            (key([0, 1, 0]), context.integer(-2)),
            (key([1, 0, 0]), context.integer(-2)),
            (key([1, 1, 0]), context.coefficient_fixture("-17/6")),
        ])
    );
    proof.replay(family.clone(), Default::default()).unwrap();
    assert!(
        proof
            .replay(test_family(["1", "1", "1"]), Default::default())
            .is_err()
    );
}

#[test]
fn unequal_active_masses_still_reject_a_denominator_swap() {
    let family = test_family(["1", "2", "1"]);
    assert!(
        TerminalSectorSymmetryWitness::verify(
            family.clone(),
            &key([1, 2, -1]),
            map(&family, [0, 1, 1, 0]),
            Default::default(),
        )
        .is_err()
    );
}

#[test]
fn a_valid_single_shear_identity_cannot_authorize_an_infinite_group_orbit() {
    let family = test_family(["1", "1", "1"]);
    let source = key([1, 0, -1]);
    // The denominator k²-1 is fixed by l -> l+k, while repeated shear images
    // of the numerator have no finite orbit. The one-map identity is valid.
    let shear = map(&family, [1, 0, 1, 1]);
    assert!(
        TerminalSectorSymmetryWitness::verify(
            family.clone(),
            &source,
            shear.clone(),
            Default::default()
        )
        .is_ok()
    );
    assert!(matches!(
        TerminalSectorSymmetryEquations::prepare_with_maps(
            family,
            &BTreeSet::from([source]),
            BTreeMap::from([(root([1, 0, -1]), vec![shear])]),
            Default::default(),
            &TerminalSectorSymmetryControl::default()
        ),
        Err(Error::InvalidWitness(
            "sector symmetry orbit generators require full-rank active momentum geometry"
        ))
    ));
}

#[test]
fn default_preparation_preserves_every_original_weak_row_without_auxiliary_sources() {
    let family = test_family(["1", "1", "1"]);
    let source = key([1, 1, -1]);
    let keys = BTreeSet::from([source.clone()]);
    let combined =
        TerminalSectorSymmetryEquations::prepare(family.clone(), &keys, Default::default())
            .unwrap();
    let weak = combined
        .legacy_supplement()
        .expect("default strong includes old original cohort");
    assert_eq!(weak.input_keys(), &keys);
    assert_eq!(weak.family_fingerprint(), family.fingerprint());
    assert!(!weak.equations(&source).is_empty());
    for row in weak.equations(&source) {
        assert!(combined.equations(&source).contains(row));
    }
    assert!(
        combined
            .equations(&source)
            .contains(combined.witnesses(&source)[0].equation())
    );
    let mut columns = keys.clone();
    for row in combined.equations(&source) {
        columns.extend(row.terms.keys().cloned());
    }
    assert_eq!(combined.inventory(), &columns);
    assert_eq!(
        combined.statistics().equations,
        combined.equations(&source).len()
    );
    assert_eq!(
        combined.statistics().output_terms,
        combined
            .equations(&source)
            .iter()
            .map(|row| row.terms.len())
            .sum::<usize>()
    );
    for auxiliary in combined.inventory().difference(&keys) {
        assert!(combined.equations(auxiliary).is_empty());
        assert!(combined.witnesses(auxiliary).is_empty());
    }
    let manual = TerminalSectorSymmetryEquations::prepare_with_maps(
        family.clone(),
        &keys,
        BTreeMap::from([(root([1, 1, -1]), vec![map(&family, [-1, 0, 0, 1])])]),
        Default::default(),
        &TerminalSectorSymmetryControl::default(),
    )
    .unwrap();
    assert!(manual.legacy_supplement().is_none());
    assert_eq!(manual.equations(&source).len(), 1);
}

#[test]
fn generator_policy_emits_exact_original_route_and_route_generator_rows_only() {
    let family = test_family(["1", "1", "1"]);
    let keys = BTreeSet::from([key([1, 2, -1]), key([-1, 2, 1])]);
    let plan = TerminalSectorSymmetryEquations::prepare_with_policy(
        family.clone(),
        &keys,
        Default::default(),
        TerminalSectorSymmetryPolicy::GeneratorRelations,
    )
    .unwrap();
    assert_eq!(
        plan.policy(),
        TerminalSectorSymmetryPolicy::GeneratorRelations
    );
    assert!(plan.legacy_supplement().is_some());
    for source in &keys {
        let source_root = Mask::try_from_indices(source.powers()).unwrap();
        let route = &plan.routes[&source_root];
        let prepared_route =
            transport::prepare_map(&family, &source_root, route.clone(), Default::default())
                .unwrap();
        let generators = &plan.generators[prepared_route.target_root()];
        let mut expected = vec![route.momentum().clone()];
        for generator in generators {
            let map = transport::compose_maps(
                &family,
                route.momentum(),
                generator.momentum(),
                Default::default(),
            )
            .unwrap();
            if !expected.contains(&map) {
                expected.push(map);
            }
        }
        assert_eq!(plan.witnesses(source).len(), expected.len());
        for (proof, map) in plan.witnesses(source).iter().zip(expected) {
            assert_eq!(proof.verified_maps().len(), 1);
            assert_eq!(proof.verified_maps()[0].momentum(), &map);
            let single = TerminalSectorSymmetryWitness::verify(
                family.clone(),
                source,
                map,
                Default::default(),
            )
            .unwrap();
            assert_eq!(proof.equation(), single.equation());
            proof.replay(family.clone(), Default::default()).unwrap();
            if !proof.equation().terms.is_empty() {
                assert!(plan.equations(source).contains(proof.equation()));
            }
        }
        for weak in plan.legacy_supplement().unwrap().equations(source) {
            assert!(plan.equations(source).contains(weak));
        }
    }
    for auxiliary in plan.inventory().difference(&keys) {
        assert!(plan.equations(auxiliary).is_empty());
        assert!(plan.witnesses(auxiliary).is_empty());
    }
}

#[test]
fn finite_generator_policy_does_not_follow_a_supplied_unipotent_map_orbit() {
    let family = test_family(["1", "1", "1"]);
    let source = key([1, 0, -1]);
    let support = root([1, 0, -1]);
    let limits = TerminalSectorSymmetryLimits::default();
    let identity = Arc::new(
        symmetry::verify(
            &family,
            &family,
            transport::identity_map(&family, limits).unwrap(),
            limits.verification,
        )
        .unwrap(),
    );
    let shear = Arc::new(
        symmetry::verify(
            &family,
            &family,
            map(&family, [1, 0, 1, 1]),
            limits.verification,
        )
        .unwrap(),
    );
    let plan = closure::prepare_with_orbit_supports_policy(
        family.clone(),
        &BTreeSet::from([source.clone()]),
        BTreeMap::from([(support.clone(), identity)]),
        BTreeMap::from([(support, vec![shear])]),
        limits,
        &TerminalSectorSymmetryControl::default(),
        TerminalSectorSymmetryPolicy::GeneratorRelations,
    )
    .unwrap();
    assert_eq!(plan.witnesses(&source).len(), 2);
    assert_eq!(plan.equations(&source).len(), 1);
    assert!(plan.legacy_supplement().is_none());
    assert_eq!(
        plan.witnesses(&source)[1].verified_maps()[0].momentum(),
        &map(&family, [1, 0, 1, 1])
    );
}

#[test]
fn a_three_image_orbit_gives_one_exact_reynolds_row_for_the_original_source() {
    let family = test_family(["1", "1", "1"]);
    let source = key([1, 2, 3]);
    // k -> l, l -> -k-l cycles all three physical denominators.
    let cycle = map(&family, [0, 1, -1, -1]);
    let plan = TerminalSectorSymmetryEquations::prepare_with_maps(
        family.clone(),
        &BTreeSet::from([source.clone()]),
        BTreeMap::from([(root([1, 2, 3]), vec![cycle])]),
        Default::default(),
        &TerminalSectorSymmetryControl::default(),
    )
    .unwrap();
    assert_eq!(
        plan.inventory(),
        &BTreeSet::from([source.clone(), key([3, 1, 2]), key([2, 3, 1])])
    );
    assert_eq!(plan.statistics().closure_keys, 3);
    assert_eq!(plan.statistics().equations, 1);
    assert_eq!(plan.statistics().analyzed_supports, 1);
    let context = family.coefficient_context();
    assert_eq!(
        plan.equations(&source)[0].terms,
        BTreeMap::from([
            (source.clone(), context.coefficient_fixture("2/3")),
            (key([3, 1, 2]), context.coefficient_fixture("-1/3")),
            (key([2, 3, 1]), context.coefficient_fixture("-1/3")),
        ])
    );
    assert_eq!(plan.witnesses(&source).len(), 1);
    assert_eq!(plan.witnesses(&source)[0].verified_maps().len(), 3);
    plan.witnesses(&source)[0]
        .replay(family.clone(), Default::default())
        .unwrap();
    for child in plan.inventory() {
        if child != &source {
            assert!(plan.equations(child).is_empty());
            assert!(plan.witnesses(child).is_empty());
        }
    }
}

#[test]
fn affine_endpoints_are_columns_of_one_complete_original_source_average() {
    let family = test_family(["1", "1", "1"]);
    let source = key([1, 1, -1]);
    let reflection = map(&family, [-1, 0, 0, 1]);
    let plan = TerminalSectorSymmetryEquations::prepare_with_maps(
        family.clone(),
        &BTreeSet::from([source.clone()]),
        BTreeMap::from([(root([1, 1, 0]), vec![reflection])]),
        Default::default(),
        &TerminalSectorSymmetryControl::default(),
    )
    .unwrap();
    assert_eq!(
        plan.inventory(),
        &BTreeSet::from([
            source.clone(),
            key([0, 1, 0]),
            key([1, 0, 0]),
            key([1, 1, 0]),
        ])
    );
    assert_eq!(plan.statistics().analyzed_supports, 1);
    assert_eq!(plan.statistics().equations, 1);
    let context = family.coefficient_context();
    assert_eq!(
        plan.equations(&source)[0].terms,
        BTreeMap::from([
            (source.clone(), context.one()),
            (key([0, 1, 0]), context.integer(-1)),
            (key([1, 0, 0]), context.integer(-1)),
            (key([1, 1, 0]), context.integer(-1)),
        ])
    );
    assert_eq!(plan.witnesses(&source).len(), 1);
    assert_eq!(plan.witnesses(&source)[0].verified_maps().len(), 2);
    plan.witnesses(&source)[0]
        .replay(family.clone(), Default::default())
        .unwrap();
    for child in plan.inventory().difference(plan.input_keys()) {
        assert!(plan.equations(child).is_empty());
        assert!(plan.witnesses(child).is_empty());
    }
    for source in plan.inventory() {
        for row in plan.equations(source) {
            assert!(row.terms.keys().all(|key| plan.inventory().contains(key)));
        }
    }
}

#[test]
fn a_two_image_high_degree_orbit_never_promotes_its_many_monomials_to_sources() {
    let family = test_family(["1", "1", "1"]);
    let source = key([1, 1, -8]);
    let progress = Cell::new(0);
    let report = |update: TerminalSectorSymmetryProgress| {
        assert!(update.processed_keys <= 1);
        progress.set(update.processed_keys);
    };
    let control = TerminalSectorSymmetryControl {
        cancelled: None,
        progress: Some(&report),
    };
    let plan = TerminalSectorSymmetryEquations::prepare_with_maps(
        family.clone(),
        &BTreeSet::from([source.clone()]),
        BTreeMap::from([(root([1, 1, 0]), vec![map(&family, [-1, 0, 0, 1])])]),
        Default::default(),
        &control,
    )
    .unwrap();
    // (2D0+2D1-D2+2)^8 has C(11,3)=165 monomials, but reflecting
    // the WHOLE polynomial again returns N^8. Its orbit has only two images.
    assert_eq!(progress.get(), 1);
    assert_eq!(plan.input_keys(), &BTreeSet::from([source.clone()]));
    assert_eq!(plan.inventory().len(), 165);
    assert_eq!(plan.statistics().analyzed_supports, 1);
    assert_eq!(plan.statistics().equations, 1);
    let row = &plan.equations(&source)[0];
    assert_eq!(row.terms.len(), 164);
    assert!(!row.terms.contains_key(&source));
    let context = family.coefficient_context();
    assert_eq!(row.terms[&key([1, 1, 0])], context.integer(-128));
    assert_eq!(row.terms[&key([1, 1, -7])], context.integer(8));
    assert_eq!(plan.witnesses(&source).len(), 1);
    assert_eq!(plan.witnesses(&source)[0].verified_maps().len(), 2);
    plan.witnesses(&source)[0]
        .replay(family.clone(), Default::default())
        .unwrap();
    for endpoint in plan.inventory().difference(plan.input_keys()) {
        assert!(plan.equations(endpoint).is_empty());
        assert!(plan.witnesses(endpoint).is_empty());
    }
}

fn cross_family_target(context: &CoefficientContext) -> Arc<IntegralFamily> {
    Arc::new(
        IntegralFamily::new(
            "sector_symmetry_cross_family",
            vec!["k".into(), "l".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    context.integer(-1),
                    vec![context.one(), context.zero(), context.zero()],
                ),
                AffineDenominator::new(
                    context.integer(-1),
                    vec![context.zero(), context.zero(), context.one()],
                ),
                AffineDenominator::new(
                    context.zero(),
                    vec![context.zero(), context.one(), context.zero()],
                ),
            ],
            vec![],
            vec![context.zero(); 3],
        )
        .unwrap(),
    )
}

#[test]
fn foreign_routes_keep_family_ownership_and_only_a_verified_round_trip_makes_a_row() {
    let family = test_family(["1", "1", "1"]);
    let target = cross_family_target(family.coefficient_context());
    let support = root([1, 1, 0]);
    let source = key([1, 2, -1]);
    let forward_map = map(&family, [0, 1, -1, 0]);
    let forward = Arc::new(
        TerminalSectorSymmetryRoute::verify(
            family.clone(),
            target.clone(),
            support.clone(),
            support.clone(),
            forward_map.clone(),
            Default::default(),
        )
        .unwrap(),
    );
    let transported = forward.transport(&source, Default::default()).unwrap();
    assert_eq!(forward.source_family_fingerprint(), family.fingerprint());
    assert_eq!(forward.target_family_fingerprint(), target.fingerprint());
    assert_eq!(
        transported.source_family_fingerprint(),
        family.fingerprint()
    );
    assert_eq!(
        transported.target_family_fingerprint(),
        target.fingerprint()
    );
    let context = family.coefficient_context();
    assert_eq!(
        transported
            .terms()
            .iter()
            .map(|term| (term.key().clone(), term.coefficient().clone()))
            .collect::<BTreeMap<_, _>>(),
        BTreeMap::from([
            (key([2, 1, -1]), context.integer(-2)),
            (key([1, 1, 0]), context.one()),
            (key([2, 0, 0]), context.one()),
            (key([2, 1, 0]), context.one()),
        ])
    );
    assert!(
        transport::complete_equation(&family, &source, &transported, Default::default()).is_err()
    );
    let foreign = Arc::new(
        symmetry::verify(&family, &target, forward_map.clone(), Default::default()).unwrap(),
    );
    assert!(transport::prepare_map(&family, &support, foreign, Default::default()).is_err());
    assert!(
        TerminalSectorSymmetryRoute::compose_round_trip(&[forward.clone()], Default::default())
            .is_err()
    );

    let backward = Arc::new(
        TerminalSectorSymmetryRoute::verify(
            target.clone(),
            family.clone(),
            support.clone(),
            support.clone(),
            map(&target, [-1, 0, 0, 1]),
            Default::default(),
        )
        .unwrap(),
    );
    let returned = TerminalSectorSymmetryRoute::compose_round_trip(
        &[forward.clone(), backward.clone()],
        Default::default(),
    )
    .unwrap();
    assert_eq!(returned.source_family_fingerprint(), family.fingerprint());
    assert_eq!(returned.target_family_fingerprint(), family.fingerprint());
    // These matrices do not commute: the substitution order gives the swap,
    // while reversing the product would give its negative.
    assert_eq!(returned.momentum(), &map(&family, [0, 1, 1, 0]));
    let mut explicit_round_trip: BTreeMap<IntegralKey, Coefficient> = BTreeMap::new();
    for first in transported.terms() {
        let returned_terms = backward.transport(first.key(), Default::default()).unwrap();
        for last in returned_terms.terms() {
            let coefficient = context
                .try_mul(first.coefficient(), last.coefficient(), Default::default())
                .unwrap();
            let previous = explicit_round_trip
                .remove(last.key())
                .unwrap_or_else(|| context.zero());
            let next = context
                .try_add(&previous, &coefficient, Default::default())
                .unwrap();
            if !next.is_zero() {
                explicit_round_trip.insert(last.key().clone(), next);
            }
        }
    }
    assert_eq!(
        explicit_round_trip,
        BTreeMap::from([(key([2, 1, -1]), context.one())])
    );
    let proof = TerminalSectorSymmetryWitness::from_verified(
        family.clone(),
        &source,
        returned,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        proof.equation().terms,
        BTreeMap::from([
            (source.clone(), context.one()),
            (key([2, 1, -1]), context.integer(-1)),
        ])
    );
    proof.replay(family.clone(), Default::default()).unwrap();
    let mismatched_support = Arc::new(
        TerminalSectorSymmetryRoute::verify(
            target.clone(),
            family,
            root([1, 0, 0]),
            root([1, 0, 0]),
            map(&target, [1, 0, 0, 1]),
            Default::default(),
        )
        .unwrap(),
    );
    assert!(
        TerminalSectorSymmetryRoute::compose_round_trip(
            &[forward.clone(), mismatched_support],
            Default::default(),
        )
        .is_err()
    );
    assert!(
        TerminalSectorSymmetryRoute::compose_round_trip(
            &[forward, backward.clone(), backward],
            Default::default(),
        )
        .is_err()
    );
}

#[test]
fn cancellation_before_work_and_from_orbit_progress_returns_no_partial_provider() {
    let family = test_family(["1", "1", "1"]);
    let keys = BTreeSet::from([key([1, 2, 3]), key([3, 1, 2])]);
    let cancelled = || true;
    let control = TerminalSectorSymmetryControl {
        cancelled: Some(&cancelled),
        progress: None,
    };
    assert!(matches!(
        TerminalSectorSymmetryEquations::prepare_with_control(
            family.clone(),
            &keys,
            Default::default(),
            &control
        ),
        Err(Error::InvalidWitness(
            "sector symmetry preparation cancelled"
        ))
    ));
    let stop = Cell::new(false);
    let callbacks = Cell::new(0);
    let cancelled = || stop.get();
    let progress = |_update: TerminalSectorSymmetryProgress| {
        callbacks.set(callbacks.get() + 1);
        stop.set(true);
    };
    let control = TerminalSectorSymmetryControl {
        cancelled: Some(&cancelled),
        progress: Some(&progress),
    };
    assert!(matches!(
        TerminalSectorSymmetryEquations::prepare_with_maps(
            family.clone(),
            &keys,
            BTreeMap::from([(root([1, 2, 3]), vec![map(&family, [0, 1, -1, -1])])]),
            Default::default(),
            &control,
        ),
        Err(Error::InvalidWitness(
            "sector symmetry preparation cancelled"
        ))
    ));
    assert!(callbacks.get() > 0);
}

#[test]
fn representative_reynolds_rows_preserve_the_full_declared_target_relation_span() {
    let family = test_family(["1", "1", "1"]);
    let left = root([1, 1, 0]);
    let right = root([0, 1, 1]);
    // Declare both dot assignments on each support. Independently average
    // every exact signed self-map and cross-support map of each source.
    let keys = BTreeSet::from([
        key([2, 1, 0]),
        key([1, 2, 0]),
        key([0, 2, 1]),
        key([0, 1, 2]),
    ]);
    let compact =
        TerminalSectorSymmetryEquations::prepare(family.clone(), &keys, Default::default())
            .unwrap();
    let mut left_maps = Vec::new();
    let mut right_maps = Vec::new();
    for first in [-1, 1] {
        for second in [-1, 1] {
            // Every signed permutation of the respective momentum bases
            // (k,l) and (l,k+l), including both orientations.
            left_maps.push((left.clone(), map(&family, [first, 0, 0, second])));
            left_maps.push((left.clone(), map(&family, [0, first, second, 0])));
            right_maps.push((
                right.clone(),
                map(&family, [second, second - first, 0, first]),
            ));
            right_maps.push((
                right.clone(),
                map(&family, [-first, second - first, first, first]),
            ));
            left_maps.push((right.clone(), map(&family, [0, first, second, second])));
            left_maps.push((right.clone(), map(&family, [first, first, 0, second])));
            right_maps.push((left.clone(), map(&family, [-first, second, first, 0])));
            right_maps.push((left.clone(), map(&family, [second, -first, 0, first])));
        }
    }
    assert_eq!(compact.inventory(), &keys);
    let context = family.coefficient_context();
    let mut reference_rows = Vec::new();
    let mut reference_inventory = keys.clone();
    for (source_root, maps) in [(left, left_maps), (right, right_maps)] {
        let routes = maps
            .into_iter()
            .map(|(target_root, map)| {
                TerminalSectorSymmetryRoute::verify(
                    family.clone(),
                    family.clone(),
                    source_root.clone(),
                    target_root,
                    map,
                    Default::default(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(routes.len(), 16);
        for source in keys
            .iter()
            .filter(|source| Mask::try_from_indices(source.powers()).unwrap() == source_root)
        {
            let mut images = BTreeSet::new();
            for route in &routes {
                let result = route.transport(source, Default::default()).unwrap();
                assert_eq!(result.terms().len(), 1);
                assert_eq!(result.terms()[0].coefficient(), &context.one());
                images.insert(result.terms()[0].key().clone());
            }
            assert_eq!(images, keys);
            reference_inventory.extend(images.iter().cloned());
            let terms = images
                .into_iter()
                .map(|image| {
                    let coefficient = if &image == source {
                        context.coefficient_fixture("3/4")
                    } else {
                        context.coefficient_fixture("-1/4")
                    };
                    (image, coefficient)
                })
                .collect();
            reference_rows.push(TerminalEquation {
                terms,
                nonzero_conditions: Vec::new(),
            });
        }
    }
    let columns = compact
        .inventory()
        .union(&reference_inventory)
        .cloned()
        .collect::<Vec<_>>();
    let matrix = |rows: &[TerminalEquation]| {
        rows.iter()
            .map(|row| {
                columns
                    .iter()
                    .map(|column| {
                        row.terms
                            .get(column)
                            .cloned()
                            .unwrap_or_else(|| context.zero())
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let compact_rows = compact
        .input_keys()
        .iter()
        .flat_map(|source| compact.equations(source).iter().cloned())
        .collect::<Vec<_>>();
    assert_eq!(compact_rows.len(), keys.len());
    let compact_matrix = matrix(&compact_rows);
    let reference_matrix = matrix(&reference_rows);
    let rank = |rows: &[Vec<Coefficient>]| {
        rank_of_coefficient_matrix(context, rows, SymbolicaCoefficientMatrixLimits::default())
            .unwrap()
            .0
    };
    let reference_rank = rank(&reference_matrix);
    assert_eq!(reference_rank, 3);
    assert_eq!(rank(&compact_matrix), reference_rank);
    let mut union = compact_matrix.clone();
    union.extend(reference_matrix);
    assert_eq!(rank(&union), reference_rank);
    // Each declared source's target relation is still implied after routing
    // and averaging, including sources on the noncanonical support.
    for pair in [
        [key([2, 1, 0]), key([1, 2, 0])],
        [key([0, 2, 1]), key([0, 1, 2])],
    ] {
        let mut augmented = compact_matrix.clone();
        augmented.push(
            columns
                .iter()
                .map(|column| {
                    if column == &pair[0] {
                        context.one()
                    } else if column == &pair[1] {
                        context.integer(-1)
                    } else {
                        context.zero()
                    }
                })
                .collect(),
        );
        assert_eq!(rank(&augmented), reference_rank);
    }
}

#[test]
fn graph_equal_supports_with_nonunit_relative_measure_stay_distinct_representatives() {
    let context = CoefficientContext::new(["d"]);
    let family = Arc::new(
        IntegralFamily::new(
            "nonunit_relative_supports",
            vec!["k".into(), "l".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            [[1, 0, 0], [0, 0, 1], [4, 4, 1]]
                .into_iter()
                .map(|row| {
                    AffineDenominator::new(
                        context.integer(-1),
                        row.map(|entry| context.integer(entry)).into(),
                    )
                })
                .collect(),
            vec![],
            vec![context.zero(); 3],
        )
        .unwrap(),
    );
    let left = root([1, 1, 0]);
    let right = root([0, 1, 1]);
    let limits = TerminalSectorSymmetryLimits::default();
    let control = TerminalSectorSymmetryControl::default();
    let mut discovery = discovery::DiscoveryCache::new(family.clone());
    let left_support = discovery
        .prepare_support(&left, limits, &control)
        .unwrap()
        .unwrap();
    let right_support = discovery
        .prepare_support(&right, limits, &control)
        .unwrap()
        .unwrap();
    // Both projector graphs are the two-line identity graph. Their bases
    // (k,l) and (l,2k+l) differ by determinant two, which forbids a unit route.
    assert_eq!(left_support.canonical_key(), right_support.canonical_key());
    assert!(
        discovery::match_supports(&left_support, &right_support, limits, &control)
            .unwrap()
            .is_none()
    );
    let plan = TerminalSectorSymmetryEquations::prepare(
        family.clone(),
        &BTreeSet::from([key([2, 1, 0]), key([0, 2, 1])]),
        limits,
    )
    .unwrap();
    assert_eq!(plan.statistics().analyzed_supports, 2);
    for source in plan.input_keys() {
        let support = Mask::try_from_indices(source.powers()).unwrap();
        assert_eq!(plan.witnesses(source).len(), 1);
        assert_eq!(plan.witnesses(source)[0].verified_maps().len(), 2);
        for generator in plan.witnesses(source)[0].verified_maps() {
            let prepared =
                transport::prepare_map(&family, &support, generator.clone(), limits).unwrap();
            assert_eq!(prepared.target_root(), &support);
        }
    }
    for source in plan.inventory() {
        let support = Mask::try_from_indices(source.powers()).unwrap();
        for row in plan.equations(source) {
            assert!(
                row.terms
                    .keys()
                    .all(|endpoint| Mask::try_from_indices(endpoint.powers()).unwrap() == support)
            );
        }
    }
}

#[test]
fn p102_full_original_identity_is_the_exact_sum_of_three_verified_complete_maps() {
    let context = CoefficientContext::new(["d"]);
    // Independent literal stored basis: a,b,h,c,q, with p=q-a-b+c.
    let momenta = [
        [-1, -1, 0, 1, 1],
        [1, 0, 0, 0, 0],
        [0, 1, 0, 0, 0],
        [0, 0, 1, 0, 1],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 1, 0],
        [0, 0, 0, 0, 1],
        [1, 1, 0, -1, 0], // N=(a+b-c)^2-1.
    ];
    let denominators = momenta
        .iter()
        .map(|momentum| {
            let mut row = Vec::new();
            for left in 0..5 {
                for right in left..5 {
                    row.push(context.integer(
                        momentum[left] * momentum[right] * if left == right { 1 } else { 2 },
                    ));
                }
            }
            AffineDenominator::new(context.integer(-1), row)
        })
        .collect();
    let family = Arc::new(
        IspCompletion::try_new(
            "P102_sector_complete_identity",
            (0..5).map(|i| format!("l{i}")).collect(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); 8],
        )
        .unwrap()
        .into_family(),
    );
    assert_eq!(family.denominator_count(), 15);
    let scalar = |pinch: Option<usize>| {
        let mut powers = vec![0; 15];
        powers[..7].fill(1);
        if let Some(axis) = pinch {
            powers[axis] = 0;
        }
        IntegralKey::try_new(powers).unwrap()
    };
    let mut numerator = scalar(None).powers().to_vec();
    numerator[7] = -1;
    let source = IntegralKey::try_new(numerator).unwrap();
    let identity = [
        [1, 0, 0, 0, 0],
        [0, 1, 0, 0, 0],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 1, 0],
        [0, 0, 0, 0, 1],
    ];
    let mut sum: BTreeMap<IntegralKey, Coefficient> = BTreeMap::new();
    for (axis, replacement) in [(0, momenta[0]), (1, momenta[0]), (3, [1, 1, 0, -1, -1])] {
        let mut rows = identity;
        rows[axis] = replacement;
        let momentum = MomentumMap::new(
            CoefficientMatrix::try_new(
                5,
                5,
                rows.into_iter()
                    .flatten()
                    .map(|value| context.integer(value)),
            )
            .unwrap(),
            CoefficientMatrix::try_new(5, 0, []).unwrap(),
            CoefficientMatrix::try_new(0, 0, []).unwrap(),
        );
        let proof = TerminalSectorSymmetryWitness::verify(
            family.clone(),
            &source,
            momentum,
            Default::default(),
        )
        .unwrap();
        proof.replay(family.clone(), Default::default()).unwrap();
        for (key, coefficient) in &proof.equation().terms {
            let previous = sum.remove(key).unwrap_or_else(|| context.zero());
            let next = context
                .try_add(&previous, coefficient, Default::default())
                .unwrap();
            if !next.is_zero() {
                sum.insert(key.clone(), next);
            }
        }
    }
    let mut expected = BTreeMap::from([
        (source.clone(), context.integer(4)),
        (scalar(Some(6)), context.integer(-2)),
        (scalar(None), context.integer(-2)),
    ]);
    for axis in [0, 1, 2, 5] {
        expected.insert(scalar(Some(axis)), context.integer(-1));
    }
    assert_eq!(sum, expected);
    // The production orbit average gives the same original identity divided
    // by four, retaining both the q pinch and the scalar mass contribution.
    let plan = TerminalSectorSymmetryEquations::prepare(
        family.clone(),
        &BTreeSet::from([source.clone()]),
        Default::default(),
    )
    .unwrap();
    let averaged = expected
        .into_iter()
        .map(|(key, coefficient)| {
            (
                key,
                context
                    .try_div(&coefficient, &context.integer(4), Default::default())
                    .unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    // Default preparation retains the three established weak equations as
    // well as the complete four-image average; neither cohort replaces the
    // other when a later saved-rule projection need not commute with it.
    assert_eq!(plan.equations(&source).len(), 4);
    assert_eq!(plan.witnesses(&source).len(), 1);
    assert_eq!(plan.witnesses(&source)[0].equation().terms, averaged);
    assert!(
        plan.equations(&source)
            .contains(plan.witnesses(&source)[0].equation())
    );
    let weak = plan.legacy_supplement().unwrap();
    assert_eq!(weak.equations(&source).len(), 3);
    for row in weak.equations(&source) {
        assert!(plan.equations(&source).contains(row));
    }
    assert_eq!(plan.witnesses(&source)[0].verified_maps().len(), 4);
    plan.witnesses(&source)[0]
        .replay(family.clone(), Default::default())
        .unwrap();
    for auxiliary in plan.inventory().difference(plan.input_keys()) {
        assert!(plan.equations(auxiliary).is_empty());
    }
}
