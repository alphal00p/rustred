use rustred::{IbpGenerator, Integral, LinearCombination};
use rustred_legacy_oracles::families::equal_mass_two_loop_vacuum;
use rustred_legacy_oracles::{
    ThreeLoopBoundaryConfig, ThreeLoopBoundaryError, ThreeLoopBoundaryReducer,
    equal_mass_three_loop_tetrahedron,
};

fn test_config() -> ThreeLoopBoundaryConfig {
    ThreeLoopBoundaryConfig {
        max_numerator_degree: 6,
        max_polynomial_terms: 10_000,
        max_polynomial_operations: 100_000,
        max_angular_terms: 100_000,
        max_tadpole_steps: 100,
        max_two_loop_dots: 4,
        max_two_loop_seed_candidates: 500,
        max_two_loop_boundary_terms: 100_000,
    }
}

fn coefficient_or_zero(
    reducer: &ThreeLoopBoundaryReducer,
    reduction: &LinearCombination,
    master: &Integral,
) -> rustred::Coefficient {
    reduction
        .coefficient(master)
        .cloned()
        .unwrap_or_else(|| reducer.family().coefficients().zero())
}

fn assert_two_master_reduction(
    reducer: &ThreeLoopBoundaryReducer,
    powers: [i32; 6],
    product: &str,
    sunset_times_tadpole: &str,
) {
    let reduction = reducer.reduce_integral(&Integral::from(powers)).unwrap();
    let coefficients = reducer.family().coefficients();
    assert_eq!(
        coefficient_or_zero(reducer, &reduction, reducer.product_master()),
        coefficients.parse(product).unwrap(),
        "wrong P3 coefficient for {powers:?}"
    );
    assert_eq!(
        coefficient_or_zero(reducer, &reduction, reducer.sunset_times_tadpole_master(),),
        coefficients.parse(sunset_times_tadpole).unwrap(),
        "wrong ST coefficient for {powers:?}"
    );
    assert!(reduction.terms().keys().all(|integral| {
        integral == reducer.product_master() || integral == reducer.sunset_times_tadpole_master()
    }));
}

fn symmetry_image(integral: &Integral, permutation: &[usize]) -> Integral {
    Integral::new(
        permutation
            .iter()
            .map(|source| integral.powers()[*source])
            .collect::<Vec<_>>(),
    )
}

fn check_tree_goldens_and_symmetries(reducer: &ThreeLoopBoundaryReducer) {
    assert_two_master_reduction(reducer, [1, 1, 1, 0, 0, 0], "1", "0");
    assert_two_master_reduction(reducer, [2, 1, 1, 0, 0, 0], "(2-d)/(2*m2)", "0");
    assert_two_master_reduction(reducer, [2, 3, 1, 0, 0, 0], "(2-d)^2*(4-d)/(16*m2^3)", "0");
    assert_two_master_reduction(reducer, [1, 1, 1, -1, 0, 0], "-m2", "0");
    assert_two_master_reduction(reducer, [1, 1, 1, -2, 0, 0], "m2^2*(d+4)/d", "0");
    assert_two_master_reduction(reducer, [1, 1, 1, -1, -1, -1], "m2^3*(8/d^2-1)", "0");
    assert_two_master_reduction(reducer, [1, 1, -1, 1, 0, 0], "-m2", "0");
    assert_two_master_reduction(reducer, [1, 1, -2, 1, 0, 0], "m2^2*(d+4)/d", "0");

    // The star and path are distinct S4 sector orbits.  Their equality is a
    // factorization identity: both active edge sets are unimodular loop bases.
    let star = Integral::from([2, 3, 4, 0, 0, 0]);
    let path = Integral::from([2, 3, 0, 4, 0, 0]);
    let reference = reducer.reduce_integral(&star).unwrap();
    assert_eq!(reducer.reduce_integral(&path).unwrap(), reference);

    // Exercise every proved tetrahedron symmetry with unequal powers, so this
    // checks full exponent-vector canonicalization rather than only sector bits.
    for source in [&star, &path] {
        let expected = reducer.reduce_integral(source).unwrap();
        for permutation in reducer.family().symmetries() {
            let image = symmetry_image(source, permutation);
            assert_eq!(
                reducer.reduce_integral(&image).unwrap(),
                expected,
                "tree symmetry failed for {source} -> {image}"
            );
        }
    }

    // Signed numerator powers use the same edge maps, but representative
    // selection must maximize the Boolean sector before its full exponent
    // vector.  Exercise all 24 images with unequal dots and numerators.
    for source in [
        Integral::from([2, 3, 4, -1, -2, 0]),
        Integral::from([2, 3, -1, 4, -2, 0]),
    ] {
        let expected = reducer.reduce_integral(&source).unwrap();
        for permutation in reducer.family().symmetries() {
            let image = symmetry_image(&source, permutation);
            assert_eq!(
                reducer.reduce_integral(&image).unwrap(),
                expected,
                "decorated tree symmetry failed for {source} -> {image}"
            );
        }
    }

    // Positive active powers are not tied to a small analytic formula table.
    let high_dot_tree = reducer
        .reduce_integral(&Integral::from([5, 6, 7, 0, 0, 0]))
        .unwrap();
    assert_eq!(high_dot_tree.len(), 1);
    assert!(
        high_dot_tree
            .coefficient(reducer.product_master())
            .is_some()
    );
}

fn check_paw_goldens_and_symmetries(reducer: &ThreeLoopBoundaryReducer) {
    assert_two_master_reduction(reducer, [1, 1, 1, 1, 0, 0], "0", "1");
    assert_two_master_reduction(reducer, [1, 2, 1, 1, 0, 0], "0", "(2-d)/(2*m2)");
    assert_two_master_reduction(reducer, [2, 1, 1, 1, 0, 0], "0", "(3-d)/(3*m2)");
    assert_two_master_reduction(
        reducer,
        [2, 1, 2, 1, 0, 0],
        "(d-2)^2/(12*m2^3)",
        "(d-2)*(d-3)/(9*m2^2)",
    );
    assert_two_master_reduction(
        reducer,
        [3, 1, 1, 1, 0, 0],
        "-(d-2)^2/(12*m2^3)",
        "(d-8)*(d-3)/(18*m2^2)",
    );
    assert_two_master_reduction(reducer, [1, 1, 1, 1, -1, 0], "1", "-m2");
    assert_two_master_reduction(
        reducer,
        [1, 1, 1, 1, -1, -1],
        "-2*m2*(d+1)/d",
        "m2^2*(d+2)/d",
    );

    let paw = Integral::from([2, 3, 2, 1, 0, 0]);
    let reference = reducer.reduce_integral(&paw).unwrap();
    for permutation in reducer.family().symmetries() {
        let image = symmetry_image(&paw, permutation);
        assert_eq!(
            reducer.reduce_integral(&image).unwrap(),
            reference,
            "paw symmetry failed for {paw} -> {image}"
        );
    }

    // The cached compatibility table stops at total dot degree four, whereas
    // the actual paw dispatch is parametric.  Authenticate the higher-dot
    // induced recurrence directly against its native E00-E01 row, verify the
    // finite table really rejects the same target, and then exercise every S4
    // routing image through the composed three-loop service.
    let induced = Integral::from([6, 2, 1]);
    assert!(matches!(
        reducer.two_loop_pipeline().reduce_integral(&induced),
        Err(
            rustred_legacy_oracles::TwoLoopPipelineError::OutOfCoverage {
                dots: 6,
                max_dots: 4,
                ..
            }
        )
    ));
    let complete = reducer.two_loop_top_dot_reducer();
    assert_eq!(complete.config().max_states, 500);
    assert_eq!(
        complete.preflight(&induced).unwrap().state_upper_bound(),
        168
    );
    complete.validate_raw_ibp_provenance(&induced).unwrap();

    let high_dot_paw = Integral::from([6, 1, 2, 1, 0, 0]);
    let high_dot_reference = reducer.reduce_integral(&high_dot_paw).unwrap();
    assert!(high_dot_reference.terms().keys().all(|integral| {
        integral == reducer.product_master() || integral == reducer.sunset_times_tadpole_master()
    }));
    for permutation in reducer.family().symmetries() {
        let image = symmetry_image(&high_dot_paw, permutation);
        assert_eq!(
            reducer.reduce_integral(&image).unwrap(),
            high_dot_reference,
            "all-dot paw S4/routing symmetry failed for {high_dot_paw} -> {image}"
        );
    }

    // Keep the arbitrary inactive-numerator path in the same routing audit.
    // These terms can pinch the induced sunset, so this also confirms that the
    // complete top-dot service still composes the old two-line formula.
    let decorated = Integral::from([2, 3, 2, 1, -1, -2]);
    let expected = reducer.reduce_integral(&decorated).unwrap();
    for permutation in reducer.family().symmetries() {
        let image = symmetry_image(&decorated, permutation);
        assert_eq!(
            reducer.reduce_integral(&image).unwrap(),
            expected,
            "decorated paw symmetry failed for {decorated} -> {image}"
        );
    }
}

fn check_typed_domain_and_resource_failures(reducer: &ThreeLoopBoundaryReducer) {
    let numerator = Integral::from([1, 1, 1, -7, 0, 0]);
    assert!(matches!(
        reducer.reduce_integral(&numerator),
        Err(ThreeLoopBoundaryError::ResourceLimit {
            resource: "numerator degree",
            requested: 7,
            limit: 6,
        })
    ));

    let banana = Integral::from([1, 1, 0, 1, 0, 1]);
    assert!(reducer.try_reduce_integral(&banana).unwrap().is_none());
    assert!(matches!(
        reducer.reduce_integral(&banana),
        Err(ThreeLoopBoundaryError::UnsupportedSector {
            integral,
            mask: 43,
        }) if integral == banana
    ));

    // A genuine sector is not expanded by the boundary code, even when it
    // carries a numerator far beyond the factorized-sector resource cap.
    let decorated_banana = Integral::from([1, 1, -100, 1, 0, 1]);
    assert!(
        reducer
            .try_reduce_integral(&decorated_banana)
            .unwrap()
            .is_none()
    );

    let wrong_arity = Integral::from([1, 1, 1]);
    assert!(matches!(
        reducer.reduce_integral(&wrong_arity),
        Err(ThreeLoopBoundaryError::WrongIntegralArity { actual: 3 })
    ));

    let too_many_tadpole_steps = Integral::from([102, 1, 1, 0, 0, 0]);
    assert!(matches!(
        reducer.reduce_integral(&too_many_tadpole_steps),
        Err(ThreeLoopBoundaryError::ResourceLimit {
            resource: "tadpole recurrence steps",
            requested: 101,
            limit: 100,
        })
    ));

    let outside_two_loop_box = Integral::from([6, 1, 1, 1, 0, 0]);
    assert!(
        reducer
            .reduce_integral(&outside_two_loop_box)
            .unwrap()
            .coefficient(reducer.sunset_times_tadpole_master())
            .is_some()
    );

    for scaleless in [
        Integral::from([0, 0, 0, 0, 0, 0]),
        Integral::from([1, 1, 0, 0, 0, 0]),
        Integral::from([2, 0, 3, 0, 0, 0]),
        Integral::from([1, 1, 0, -100, 0, 0]),
    ] {
        assert!(
            reducer.reduce_integral(&scaleless).unwrap().is_zero(),
            "{scaleless} should be scaleless"
        );
    }

    assert!(matches!(
        ThreeLoopBoundaryReducer::new(equal_mass_two_loop_vacuum().unwrap(), test_config()),
        Err(ThreeLoopBoundaryError::WrongLoopCount { actual: 2 })
    ));

    let term_limited = ThreeLoopBoundaryConfig {
        max_polynomial_terms: 1,
        ..test_config()
    };
    let term_limited =
        ThreeLoopBoundaryReducer::new(equal_mass_three_loop_tetrahedron().unwrap(), term_limited)
            .unwrap();
    assert!(matches!(
        term_limited.reduce_integral(&Integral::from([1, 1, 1, -1, 0, 0])),
        Err(ThreeLoopBoundaryError::ResourceLimit {
            resource: "polynomial term upper bound",
            requested: 7,
            limit: 1,
        })
    ));

    let operation_limited = ThreeLoopBoundaryConfig {
        max_polynomial_operations: 1,
        ..test_config()
    };
    let operation_limited = ThreeLoopBoundaryReducer::new(
        equal_mass_three_loop_tetrahedron().unwrap(),
        operation_limited,
    )
    .unwrap();
    assert!(matches!(
        operation_limited.reduce_integral(&Integral::from([1, 1, 1, -1, 0, 0])),
        Err(ThreeLoopBoundaryError::ResourceLimit {
            resource: "polynomial expansion operations",
            requested: 4,
            limit: 1,
        })
    ));

    let angular_limited = ThreeLoopBoundaryConfig {
        max_angular_terms: 1,
        ..test_config()
    };
    let angular_limited = ThreeLoopBoundaryReducer::new(
        equal_mass_three_loop_tetrahedron().unwrap(),
        angular_limited,
    )
    .unwrap();
    assert!(matches!(
        angular_limited.reduce_integral(&Integral::from([1, 1, 1, -1, 0, 0])),
        Err(ThreeLoopBoundaryError::ResourceLimit {
            resource: "angular contraction terms",
            requested,
            limit: 1,
        }) if requested > 1
    ));

    // The complete reducer has an independent whole-DAG state guard.  A
    // budget of 100 still constructs the legacy D=4 table (96 candidates),
    // but rejects the D=6 induced target before normal-form coefficient work.
    let state_limited = ThreeLoopBoundaryReducer::new(
        equal_mass_three_loop_tetrahedron().unwrap(),
        ThreeLoopBoundaryConfig {
            max_two_loop_seed_candidates: 100,
            ..test_config()
        },
    )
    .unwrap();
    assert!(matches!(
        state_limited.reduce_integral(&Integral::from([6, 1, 2, 1, 0, 0])),
        Err(ThreeLoopBoundaryError::TwoLoopTopDot(
            rustred_legacy_oracles::TwoLoopTopDotError::ResourceLimit {
                resource: "normal-form state upper bound",
                requested: 168,
                limit: 100,
            }
        ))
    ));

    let coefficient_degree_limited = ThreeLoopBoundaryReducer::new(
        equal_mass_three_loop_tetrahedron().unwrap(),
        ThreeLoopBoundaryConfig {
            max_tadpole_steps: 1_000_000,
            ..test_config()
        },
    )
    .unwrap();
    for powers in [[65_537, 1, 1, 0, 0, 0], [1, 65_537, 1, 1, 0, 0]] {
        assert!(matches!(
            coefficient_degree_limited.reduce_integral(&Integral::from(powers)),
            Err(ThreeLoopBoundaryError::ResourceLimit {
                resource: "Symbolica coefficient exponent degree",
                requested: 65_536,
                limit: 65_535,
            })
        ));
    }
}

fn assert_supported_identity(
    reducer: &ThreeLoopBoundaryReducer,
    equation: &LinearCombination,
    seed: &Integral,
    differentiated_loop: usize,
    contraction_loop: usize,
) {
    for integral in equation.terms().keys() {
        assert!(
            reducer.try_reduce_integral(integral).unwrap().is_some(),
            "selected boundary IBP escaped the scalar domain: seed {seed}, derivative {differentiated_loop}, contraction {contraction_loop}, term {integral}"
        );
    }
    let remainder = reducer.reduce_combination(equation).unwrap();
    assert!(
        remainder.is_zero(),
        "boundary formula violates raw IBP for seed {seed}, derivative {differentiated_loop}, contraction {contraction_loop}: {remainder:?}"
    );
}

fn check_supported_raw_ibps(reducer: &ThreeLoopBoundaryReducer) {
    let generator = IbpGenerator::new(reducer.family());
    let mut checked = 0_usize;

    // In the star representative, the three diagonal derivatives are three
    // independent one-loop tadpole IBPs and never introduce an inactive line.
    for a in 1..=3 {
        for b in 1..=3 {
            for c in 1..=3 {
                let seed = Integral::from([a, b, c, 0, 0, 0]);
                for identity in generator
                    .generate_raw(&seed)
                    .into_iter()
                    .filter(|identity| identity.differentiated_loop == identity.contraction_loop)
                {
                    assert_supported_identity(
                        reducer,
                        &identity.equation,
                        &seed,
                        identity.differentiated_loop,
                        identity.contraction_loop,
                    );
                    checked += 1;
                }
            }
        }
    }

    // In the canonical paw, k2 is the bridge.  d/dk2.k2 checks its tadpole
    // factor, while the four identities with both vector labels in {k1,k3}
    // are precisely the embedded two-loop sunset IBPs.  No selected identity
    // contains D5 or D6, so every term remains in this module's exact domain.
    for a in 1..=2 {
        for b in 1..=2 {
            for c in 1..=2 {
                for e in 1..=2 {
                    let seed = Integral::from([a, b, c, e, 0, 0]);
                    for identity in generator
                        .generate_raw(&seed)
                        .into_iter()
                        .filter(|identity| {
                            let left = identity.differentiated_loop;
                            let right = identity.contraction_loop;
                            (left == 1 && right == 1)
                                || ([0, 2].contains(&left) && [0, 2].contains(&right))
                        })
                    {
                        assert_supported_identity(
                            reducer,
                            &identity.equation,
                            &seed,
                            identity.differentiated_loop,
                            identity.contraction_loop,
                        );
                        checked += 1;
                    }
                }
            }
        }
    }

    // Repeat the five exactly factor-preserving paw identities at a seed whose
    // induced sunset lies beyond the retained finite two-loop table.  Their
    // vanishing is an independent native-IBP check of the integrated all-dot
    // normal form, not merely a comparison with that normal form's API.
    let high_dot_seed = Integral::from([5, 1, 2, 1, 0, 0]);
    for identity in generator
        .generate_raw(&high_dot_seed)
        .into_iter()
        .filter(|identity| {
            let left = identity.differentiated_loop;
            let right = identity.contraction_loop;
            (left == 1 && right == 1) || ([0, 2].contains(&left) && [0, 2].contains(&right))
        })
    {
        assert_supported_identity(
            reducer,
            &identity.equation,
            &high_dot_seed,
            identity.differentiated_loop,
            identity.contraction_loop,
        );
        checked += 1;
    }

    assert_eq!(checked, 27 * 3 + 16 * 5 + 5);

    // Once inactive numerator powers are supported, every one of the nine raw
    // identities remains inside a factorized or scaleless boundary.  These
    // seeds exercise each inactive tree chord and both paw numerators without
    // relying on the formula used to derive the reducer.
    let decorated_seeds = [
        Integral::from([1, 1, 1, -1, 0, 0]),
        Integral::from([1, 1, 1, 0, -1, 0]),
        Integral::from([1, 1, 1, 0, 0, -1]),
        Integral::from([1, 1, -1, 1, 0, 0]),
        Integral::from([1, 1, 0, 1, -1, 0]),
        Integral::from([1, 1, 0, 1, 0, -1]),
        Integral::from([1, 1, 1, 1, -1, 0]),
        Integral::from([1, 1, 1, 1, 0, -1]),
        Integral::from([1, 1, 1, 1, -1, -1]),
        // Rank-four mixed bridge contraction: this independently exercises
        // multiple cross-pair multiplicities rather than only the rank-two
        // cases used by the closed-form goldens above.
        Integral::from([1, 1, 1, 1, -2, -2]),
    ];
    let mut decorated_checked = 0_usize;
    for seed in decorated_seeds {
        for identity in generator.generate_raw(&seed) {
            assert_supported_identity(
                reducer,
                &identity.equation,
                &seed,
                identity.differentiated_loop,
                identity.contraction_loop,
            );
            decorated_checked += 1;
        }
    }
    assert_eq!(decorated_checked, 10 * 9);
}

// Restricted Symbolica binds an instance to the first OS thread that enters
// it.  Keep construction, exact goldens, symmetries, and IBP checks together.
#[test]
fn exact_scalar_three_loop_boundary_slice() {
    let reducer =
        ThreeLoopBoundaryReducer::new(equal_mass_three_loop_tetrahedron().unwrap(), test_config())
            .unwrap();

    assert_eq!(
        reducer.product_master(),
        &Integral::from([1, 1, 1, 0, 0, 0])
    );
    assert_eq!(
        reducer.sunset_times_tadpole_master(),
        &Integral::from([1, 1, 1, 1, 0, 0])
    );
    assert_eq!(reducer.two_loop_pipeline().config().max_dots, 4);
    assert_eq!(reducer.two_loop_pipeline().config().max_numerator_degree, 6);
    assert_eq!(
        reducer.two_loop_top_dot_reducer().family().fingerprint(),
        reducer.two_loop_pipeline().family().fingerprint()
    );

    check_tree_goldens_and_symmetries(&reducer);
    check_paw_goldens_and_symmetries(&reducer);
    check_typed_domain_and_resource_failures(&reducer);
    check_supported_raw_ibps(&reducer);
}
