#![cfg(feature = "legacy-authored-oracles")]

use rustred::families::equal_mass_two_loop_vacuum;
use rustred::two_loop::{TwoLoopBoundaryConfig, TwoLoopBoundaryError, TwoLoopBoundaryReducer};
use rustred::{IbpGenerator, Integral, LinearCombination};

fn assert_reduction(reducer: &TwoLoopBoundaryReducer<'_>, powers: [i32; 3], expected: &str) {
    let result = reducer.reduce_integral(&Integral::from(powers)).unwrap();
    let expected = reducer.family().coefficients().parse(expected).unwrap();
    assert_eq!(result.len(), 1, "unexpected reduction for {powers:?}");
    assert_eq!(
        result.coefficient(reducer.master()),
        Some(&expected),
        "wrong reduction for {powers:?}"
    );
}

fn check_golden_reductions(reducer: &TwoLoopBoundaryReducer<'_>) {
    // Section 8.6 uses D=k^2-s, whereas the built-in Euclidean family uses
    // D=k^2+m2.  Thus s=-m2 in the odd-power answers and tadpole recurrence.
    assert_reduction(reducer, [0, 2, 1], "(2-d)/(2*m2)");
    assert_reduction(reducer, [0, 2, 2], "(d-2)^2/(4*m2^2)");
    assert_reduction(reducer, [-1, 1, 1], "-m2");
    assert_reduction(reducer, [-2, 1, 1], "m2^2*(d+4)/d");
}

fn check_permutations_and_sector_boundaries(reducer: &TwoLoopBoundaryReducer<'_>) {
    let permutations = [
        [-2, 1, 2],
        [-2, 2, 1],
        [1, -2, 2],
        [2, -2, 1],
        [1, 2, -2],
        [2, 1, -2],
    ];
    let reference = reducer
        .reduce_integral(&Integral::from(permutations[0]))
        .unwrap();
    for powers in permutations {
        assert_eq!(
            reducer.reduce_integral(&Integral::from(powers)).unwrap(),
            reference,
            "permutation was not canonicalized for {powers:?}"
        );
    }

    for powers in [[0, 0, 0], [-4, -2, 0], [-3, 0, 2], [0, 4, 0]] {
        assert!(
            reducer
                .reduce_integral(&Integral::from(powers))
                .unwrap()
                .is_zero(),
            "{powers:?} should be scaleless"
        );
    }

    let top = Integral::from([1, 1, 1]);
    assert!(reducer.try_reduce_integral(&top).unwrap().is_none());
    assert_eq!(
        reducer.reduce_integral(&top),
        Err(TwoLoopBoundaryError::TopSector(top))
    );

    // Both large dot and large numerator inputs cross the hard `u16`
    // coefficient-degree ceiling.  The direct boundary API must reject them
    // before allocating a recurrence window or a mass-power cache.
    for powers in [[0, 65_537, 1], [-65_536, 1, 1]] {
        assert!(matches!(
            reducer.reduce_integral(&Integral::from(powers)),
            Err(TwoLoopBoundaryError::CoefficientExponentLimit {
                requested: 65_536,
                limit: 65_535,
            })
        ));
    }

    // Degree 100 is representable, but the dense direct formula has a cubic
    // iteration estimate and is rejected before allocating its caches.
    assert!(matches!(
        reducer.reduce_integral(&Integral::from([-100, 1, 1])),
        Err(TwoLoopBoundaryError::ResourceLimit {
            resource: "boundary formula iteration estimate",
            requested: 1_030_303,
            limit: 1_000_000,
        })
    ));
}

fn check_bounded_formula_against_raw_ibps(reducer: &TwoLoopBoundaryReducer<'_>) {
    let generator = IbpGenerator::new(reducer.family());

    // These raw identities are generated from momentum derivatives and do not
    // use the boundary formula.  With a<=0 every generated term remains in a
    // pair/single/empty sector, so exact cancellation is an independent,
    // bounded cross-check of arbitrary numerator and tadpole powers.
    for a in -2..=0 {
        for b in 1..=3 {
            for c in 1..=3 {
                let seed = Integral::from([a, b, c]);
                for identity in generator.generate_raw(&seed) {
                    let mut remainder = LinearCombination::new();
                    for (integral, coefficient) in identity.equation.terms() {
                        let reduced = reducer.reduce_integral(integral).unwrap();
                        remainder.add_scaled(&reduced, coefficient);
                    }
                    assert!(
                        remainder.is_zero(),
                        "boundary formula violates raw IBP for seed {seed}, derivative {}, contraction {}: {remainder:?}",
                        identity.differentiated_loop,
                        identity.contraction_loop,
                    );
                }
            }
        }
    }
}

// Restricted Symbolica binds an instance to the first OS thread that enters
// it.  Keep every Symbolica integration check in a single test/thread.
#[test]
fn exact_two_loop_boundary_reduction() {
    let family = equal_mass_two_loop_vacuum().unwrap();
    let reducer = TwoLoopBoundaryReducer::new(&family).unwrap();

    assert_eq!(reducer.master(), &Integral::from([0, 1, 1]));
    check_golden_reductions(&reducer);
    check_permutations_and_sector_boundaries(&reducer);
    check_bounded_formula_against_raw_ibps(&reducer);

    let tightly_bounded = TwoLoopBoundaryReducer::new_with_config(
        &family,
        TwoLoopBoundaryConfig {
            max_formula_iterations: 2,
        },
    )
    .unwrap();
    assert_eq!(tightly_bounded.config().max_formula_iterations, 2);
    assert!(matches!(
        tightly_bounded.reduce_integral(&Integral::from([0, 1, 1])),
        Err(TwoLoopBoundaryError::ResourceLimit {
            resource: "boundary formula iteration estimate",
            requested: 3,
            limit: 2,
        })
    ));
}
