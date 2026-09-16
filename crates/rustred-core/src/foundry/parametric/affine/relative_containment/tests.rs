use super::*;
use crate::algebra::CoefficientContext;

fn raw(
    context: &CoefficientContext,
    equations: &[&str],
    sector: &[bool],
    indices: &[usize],
) -> AffineApplicationDomain {
    AffineApplicationDomain {
        sector: sector.to_vec().into_boxed_slice(),
        fixed: vec![None; sector.len()].into_boxed_slice(),
        indices: indices.to_vec().into_boxed_slice(),
        equations: equations
            .iter()
            .map(|s| context.coefficient_fixture(s).numerator)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        primitive_matrix: None,
        integral_chart: None,
    }
}

fn domain(equations: &[&str]) -> AffineApplicationDomain {
    raw(
        &CoefficientContext::new(["d", "a", "b", "c"]),
        equations,
        &[false; 3],
        &[1, 2, 3],
    )
}

fn entire() -> LatticeBox {
    LatticeBox::try_new([0; 3], [None; 3]).unwrap()
}

#[test]
fn captured_fg115_exclusion_is_relative_to_target_not_its_box_hull() {
    let context = CoefficientContext::new([
        "d", "n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9",
    ]);
    let sector = [
        true, true, false, false, true, true, true, false, false, false,
    ];
    let indices: Vec<_> = (1..=10).collect();
    let mut target = raw(&context, &["1+n2+n9"], &sector, &indices);
    target.fixed = vec![
        Some(1),
        Some(1),
        None,
        Some(0),
        Some(1),
        Some(1),
        Some(1),
        Some(0),
        None,
        None,
    ]
    .into();
    let exclusions: Vec<_> = ["1+n8+n9", "3+n8+2*n9", "n8", "n2"]
        .map(|equation| {
            let mut domain = raw(&context, &["1+n2+n9", equation], &sector, &indices);
            domain.fixed = target.fixed.clone();
            Arc::new(domain)
        })
        .into();
    let piece = |lo, hi| {
        LatticeBox::try_new(
            [0, 0, 1, 0, 0, 0, 0, 0, lo, 0],
            [
                Some(0),
                Some(0),
                None,
                Some(0),
                Some(0),
                Some(0),
                Some(0),
                Some(0),
                Some(hi),
                Some(0),
            ],
        )
        .unwrap()
    };
    assert!(!exclusions[0].is_proved_to_contain_box(&piece(1, 1)));
    assert!(
        target
            .is_proved_excluded_in_box(&piece(1, 1), &exclusions, Default::default())
            .unwrap()
    );
    assert!(
        !target
            .is_proved_excluded_in_box(&piece(2, 2), &exclusions, Default::default())
            .unwrap()
    );
    assert!(
        !target
            .is_proved_excluded_in_box(&piece(1, 2), &exclusions, Default::default())
            .unwrap()
    );
    let coefficient = context.coefficient_fixture("-(1+n8)/((1+n8+n9)*(3+n8+2*n9))");
    let restrict = |polynomial: &CoefficientPolynomial, value| {
        polynomial
            .replace(9, &Integer::from(value))
            .replace(10, &Integer::zero())
    };
    assert!(restrict(&coefficient.numerator, -1).is_zero());
    assert!(restrict(&coefficient.denominator, -1).is_zero());
    assert_eq!(
        restrict(&coefficient.numerator, -2).get_constant(),
        -restrict(&coefficient.denominator, -2).get_constant()
    );
}

#[test]
fn every_sibling_of_one_exclusion_is_required_and_or_branches_stay_separate() {
    let target = domain(&["a-b"]);
    assert!(
        target
            .is_proved_excluded_in_box(
                &entire(),
                &[Arc::new(domain(&["2*a-2*b"]))],
                Default::default()
            )
            .unwrap()
    );
    let exclusions = [
        Arc::new(domain(&["a-b", "a-c"])),
        Arc::new(domain(&["a-c-1", "2*a-2*b"])),
    ];
    assert!(
        !target
            .is_proved_excluded_in_box(&entire(), &exclusions, Default::default())
            .unwrap()
    );
}

#[test]
fn augmented_inconsistency_is_not_containment_but_base_inconsistency_is_empty() {
    let target = domain(&["a-b"]);
    assert!(
        !target
            .is_proved_excluded_in_box(
                &entire(),
                &[Arc::new(domain(&["a-b-1"]))],
                Default::default()
            )
            .unwrap()
    );
    let impossible = domain(&["a-b", "a-b-1"]);
    assert!(
        impossible
            .is_proved_excluded_in_box(&entire(), &[Arc::new(domain(&["a-c"]))], Default::default())
            .unwrap()
    );
}

#[test]
fn fixed_faces_are_not_inferred_from_a_rational_chart() {
    let target = domain(&["a-b"]);
    let mut exclusion = domain(&["a-b"]);
    exclusion.fixed[2] = Some(-1);
    assert!(
        !target
            .is_proved_excluded_in_box(
                &entire(),
                &[Arc::new(exclusion.clone())],
                Default::default()
            )
            .unwrap()
    );
    let face = LatticeBox::try_new([0, 0, 1], [None, None, Some(1)]).unwrap();
    assert!(
        target
            .is_proved_excluded_in_box(&face, &[Arc::new(exclusion.clone())], Default::default())
            .unwrap()
    );
    assert!(
        !exclusion
            .is_proved_excluded_in_box(&entire(), &[Arc::new(target)], Default::default())
            .unwrap()
    );
}

#[test]
fn singleton_integer_contradictions_do_not_assume_integral_free_parameters() {
    let target = domain(&["2*a-b"]);
    let exclusion = [Arc::new(domain(&["a+1"]))];
    let fixed_b = |b| LatticeBox::try_new([0, b, 0], [None, Some(b), None]).unwrap();
    assert!(
        target
            .is_proved_excluded_in_box(&fixed_b(2), &exclusion, Default::default())
            .unwrap()
    );
    assert!(
        target
            .is_proved_excluded_in_box(&fixed_b(1), &exclusion, Default::default())
            .unwrap()
    );
    assert!(
        !target
            .is_proved_excluded_in_box(&entire(), &exclusion, Default::default())
            .unwrap()
    );
}

#[test]
fn native_wide_singletons_include_positive_two_to_the_64_without_narrowing() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let piece = LatticeBox::try_new(
        [u64::MAX, 0, u64::MAX],
        [Some(u64::MAX), None, Some(u64::MAX)],
    )
    .unwrap();
    for sector in [[true; 3], [false; 3]] {
        let target = raw(&context, &["a-b"], &sector, &[1, 2, 3]);
        let excluded = raw(&context, &["b-c"], &sector, &[1, 2, 3]);
        assert!(
            target
                .is_proved_excluded_in_box(&piece, &[Arc::new(excluded)], Default::default())
                .unwrap()
        );
        let wrong = raw(&context, &["b-c-1"], &sector, &[1, 2, 3]);
        assert!(
            !target
                .is_proved_excluded_in_box(&piece, &[Arc::new(wrong)], Default::default())
                .unwrap()
        );
    }
    assert_eq!(physical_bits(u64::MAX, true), 65);
    assert_eq!(physical_bits(u64::MAX, false), 64);
}

#[test]
fn physical_axes_and_native_variable_maps_must_match() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let target = raw(&context, &["a-b"], &[false; 3], &[3, 1, 2]);
    let exclusion = Arc::new(raw(&context, &["a-b"], &[false; 3], &[3, 1, 2]));
    assert!(
        target
            .is_proved_excluded_in_box(&entire(), &[exclusion], Default::default())
            .unwrap()
    );
    for indices in [[1, 2, 3], [3, 3, 2], [3, 1, 99]] {
        let exclusion = raw(&context, &["a-b"], &[false; 3], &indices);
        assert!(
            !target
                .is_proved_excluded_in_box(&entire(), &[Arc::new(exclusion)], Default::default())
                .unwrap()
        );
    }
    let foreign = CoefficientContext::new(["q", "x", "y", "z"]);
    let exclusion = raw(&foreign, &["x-y"], &[false; 3], &[3, 1, 2]);
    assert!(
        !target
            .is_proved_excluded_in_box(&entire(), &[Arc::new(exclusion)], Default::default())
            .unwrap()
    );
}

#[test]
fn all_original_siblings_are_admitted_before_shortcuts() {
    let target = domain(&["a-b"]);
    let point = LatticeBox::try_new([0; 3], [Some(0); 3]).unwrap();
    let good = Arc::new(domain(&["a-b"]));
    for equations in [vec!["a*b"], vec!["d*a"], vec!["a-b", "c^2"]] {
        let bad = Arc::new(domain(&equations));
        let mut calls = 0;
        assert!(
            !target
                .excluded_using(
                    &point,
                    &[good.clone(), bad],
                    Default::default(),
                    |_, _, _| {
                        calls += 1;
                        Ok(Some(0))
                    }
                )
                .unwrap()
        );
        assert_eq!(calls, 0);
    }
    let mut malformed = domain(&["a-b"]);
    malformed.equations[0].exponents.pop();
    assert!(
        !target
            .is_proved_excluded_in_box(
                &point,
                &[good.clone(), Arc::new(malformed)],
                Default::default()
            )
            .unwrap()
    );
    let mut oversized = domain(&["a-b"]);
    oversized.equations = vec![oversized.equations[0].clone(); MAX_EQUATIONS].into();
    assert_eq!(
        target.is_proved_excluded_in_box(&point, &[good, Arc::new(oversized)], Default::default()),
        Err("relative affine equations")
    );
    let mut mismatched = domain(&["a-b"]);
    mismatched.sector[0] = true;
    assert!(
        !target
            .is_proved_excluded_in_box(&point, &[Arc::new(mismatched)], Default::default())
            .unwrap()
    );
}

#[test]
fn equation_cap_counts_the_base_once_and_the_complete_augmented_system() {
    let mut target = domain(&["a-b"]);
    let branches = [Arc::new(domain(&["a-b"]))];
    target.equations = vec![target.equations[0].clone(); MAX_EQUATIONS - 1].into();
    assert!(
        target
            .is_proved_excluded_in_box(&entire(), &branches, Default::default())
            .unwrap()
    );
    target.equations = vec![target.equations[0].clone(); MAX_EQUATIONS].into();
    assert_eq!(
        target.is_proved_excluded_in_box(&entire(), &branches, Default::default()),
        Err("relative affine equations")
    );
}

#[test]
fn shared_work_precharges_exact_boundary_and_never_resets_for_an_exclusion() {
    let target = domain(&["a-b"]);
    let same = Arc::new(domain(&["a-b"]));
    let mut limits = IndexedGuardLimits {
        max_exact_hyperplane_replay_work: 44,
        ..Default::default()
    };
    assert!(
        target
            .is_proved_excluded_in_box(&entire(), &[same.clone()], limits)
            .unwrap()
    );
    limits.max_exact_hyperplane_replay_work = 43;
    assert_eq!(
        target.is_proved_excluded_in_box(&entire(), &[same.clone()], limits),
        Err("relative affine work")
    );
    let branches = [Arc::new(domain(&["a-c"])), same];
    limits.max_exact_hyperplane_replay_work = 72;
    assert!(
        target
            .is_proved_excluded_in_box(&entire(), &branches, limits)
            .unwrap()
    );
    limits.max_exact_hyperplane_replay_work = 71;
    assert_eq!(
        target.is_proved_excluded_in_box(&entire(), &branches, limits),
        Err("relative affine work")
    );
}

#[test]
fn bit_term_and_substitution_caps_precede_the_corresponding_native_work() {
    let target = domain(&["a-b"]);
    let branches = [Arc::new(domain(&["a-b"]))];
    let point = LatticeBox::try_new([u64::MAX; 3], [Some(u64::MAX); 3]).unwrap();
    let mut calls = 0;
    let limits = IndexedGuardLimits {
        max_total_integer_bits: 200,
        ..Default::default()
    };
    assert!(
        target
            .excluded_using(&point, &branches, limits, |_, _, _| {
                calls += 1;
                Ok(Some(1))
            })
            .is_err()
    );
    assert_eq!(calls, 0);
    for limits in [
        IndexedGuardLimits {
            max_input_terms: 3,
            ..Default::default()
        },
        IndexedGuardLimits {
            max_exact_hyperplane_replay_substitutions: 0,
            ..Default::default()
        },
        IndexedGuardLimits {
            max_exact_hyperplane_replay_terms: 0,
            ..Default::default()
        },
        IndexedGuardLimits {
            max_exact_hyperplane_replay_work: 0,
            ..Default::default()
        },
    ] {
        assert!(
            target
                .excluded_using(&point, &branches, limits, |_, _, _| {
                    calls += 1;
                    Ok(Some(1))
                })
                .is_err()
        );
        assert_eq!(calls, 0);
    }
}

#[test]
fn native_errors_and_panics_never_supply_a_proof() {
    let target = domain(&["a-b"]);
    let branches = [Arc::new(domain(&["a-b"]))];
    assert!(
        !target
            .excluded_using(&entire(), &branches, Default::default(), |_, _, _| Err(()))
            .unwrap()
    );
    let mut calls = 0;
    assert!(
        !target
            .excluded_using(&entire(), &branches, Default::default(), |_, _, _| {
                calls += 1;
                if calls == 1 {
                    Ok(Some(1))
                } else {
                    panic!("injected native failure")
                }
            })
            .unwrap()
    );
    assert_eq!(calls, 2);
}
