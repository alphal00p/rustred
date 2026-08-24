#![cfg(feature = "legacy-authored-oracles")]

use std::cmp::Ordering;

use rustred::families::equal_mass_two_loop_vacuum;
use rustred::{
    Integral, THREE_LOOP_TOP_DOT_IBP_WEIGHT_NUMERATORS, ThreeLoopTopDotConfig,
    ThreeLoopTopDotError, ThreeLoopTopDotReducer,
};

#[test]
fn certified_three_loop_top_dot_recurrence() {
    let reducer = ThreeLoopTopDotReducer::build(ThreeLoopTopDotConfig::default()).unwrap();
    assert_eq!(
        THREE_LOOP_TOP_DOT_IBP_WEIGHT_NUMERATORS,
        [[3, -4, 2], [6, -1, -4], [0, 2, -1]]
    );

    // These asymmetric points prevent symmetry collection from concealing a
    // wrong edge/index map.  The all-twos point guards the canonical-order
    // counterexample which invalidates a simpler single-row recurrence.
    for target in [
        Integral::from([3, 2, 1, 4, 2, 1]),
        Integral::from([5, 1, 3, 2, 4, 2]),
        Integral::from([2, 2, 2, 2, 2, 2]),
    ] {
        reducer.validate_raw_ibp_provenance(&target).unwrap();
        assert_eq!(
            reducer.weighted_raw_ibp(&target).unwrap(),
            reducer.expected_weighted_raw_ibp(&target).unwrap()
        );
    }
    let family = reducer.family();

    for encoded in 0_u32..3_u32.pow(6) {
        let mut value = encoded;
        let powers = (0..6)
            .map(|_| {
                let power = i32::try_from(value % 3 + 1).unwrap();
                value /= 3;
                power
            })
            .collect::<Vec<_>>();
        let input = Integral::new(powers);
        let canonical = family.canonicalize(&input).unwrap();
        let Some(rewrite) = reducer.rewrite_once(&input).unwrap() else {
            assert_eq!(canonical, Integral::from([1, 1, 1, 1, 1, 1]));
            continue;
        };

        assert_eq!(rewrite.target(), &canonical);
        assert!(rewrite.target().powers()[0] > 1);
        assert!(rewrite.rhs().terms().keys().all(|output| {
            output.numerator_degree() == 0
                && family.compare_integrals(output, rewrite.target()) == Ordering::Less
        }));
        for output in rewrite.rhs().terms().keys() {
            if output.denominator_count() == 6 {
                assert_eq!(
                    output.dot_degree() + 1,
                    rewrite.target().dot_degree(),
                    "top-sector branch did not lower total dot degree: {:?}",
                    rewrite
                );
            }
        }
    }

    // Keep the earlier ordering counterexample explicit and easy to diagnose.
    let all_twos = reducer
        .rewrite_once(&Integral::from([2, 2, 2, 2, 2, 2]))
        .unwrap()
        .unwrap();
    assert!(
        all_twos.rhs().terms().keys().all(|output| {
            family.compare_integrals(output, all_twos.target()) == Ordering::Less
        })
    );

    // At D=2 the step is intentionally not a full reduction: proper dotted
    // five-line sectors survive, even though every branch is strictly lower.
    let d2 = reducer
        .rewrite_once(&Integral::from([2, 2, 1, 1, 1, 1]))
        .unwrap()
        .unwrap();
    let dotted_five_line = d2
        .rhs()
        .terms()
        .keys()
        .filter(|output| output.denominator_count() == 5 && output.dot_degree() > 0)
        .count();
    assert_eq!(dotted_five_line, 3);

    let context = reducer.family().coefficients();
    let corner = Integral::from([1, 1, 1, 1, 1, 1]);
    let rewrite = reducer
        .rewrite_once(&Integral::from([1, 1, 2, 1, 1, 1]))
        .unwrap()
        .unwrap();

    assert_eq!(rewrite.target(), &Integral::from([2, 1, 1, 1, 1, 1]));
    assert_eq!(rewrite.rhs().len(), 1);
    assert_eq!(
        rewrite.rhs().coefficient(&corner),
        Some(&context.parse("(4-d)/(4*m2)").unwrap())
    );

    let corner = Integral::from([1, 1, 1, 1, 1, 1]);
    assert_eq!(reducer.rewrite_once(&corner).unwrap(), None);
    assert!(matches!(
        reducer.validate_raw_ibp_provenance(&corner),
        Err(ThreeLoopTopDotError::PivotGuardNotSatisfied { first_power: 1, .. })
    ));

    assert!(matches!(
        reducer.rewrite_once(&Integral::from([1, 1, 1, 1, 1])),
        Err(ThreeLoopTopDotError::WrongIntegralArity {
            expected: 6,
            actual: 5,
        })
    ));
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([2, 1, 1, 1, 0, 1])),
        Err(ThreeLoopTopDotError::OutsideScalarTopSector {
            position: 4,
            power: 0,
            ..
        })
    ));
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([2, 1, -1, 1, 1, 1])),
        Err(ThreeLoopTopDotError::OutsideScalarTopSector {
            position: 2,
            power: -1,
            ..
        })
    ));

    let capped = ThreeLoopTopDotReducer::build(ThreeLoopTopDotConfig {
        max_output_terms: 16,
    })
    .unwrap();
    assert!(matches!(
        capped.rewrite_once(&Integral::from([2, 1, 1, 1, 1, 1])),
        Err(ThreeLoopTopDotError::ResourceLimit {
            resource: "explicit recurrence terms",
            requested: 17,
            limit: 16,
        })
    ));

    // Canonicalization places the extreme exponent on line one.  The explicit
    // recurrence's +1 shifts on another line then fail before any wrapping.
    let extreme = Integral::from([i32::MAX, i32::MAX, 1, 1, 1, 1]);
    assert!(matches!(
        reducer.rewrite_once(&extreme),
        Err(ThreeLoopTopDotError::ExponentOverflow { .. })
    ));

    // The legacy family comparator saturates total dot degree at u32::MAX.
    // This recurrence's local certificate keeps the full six-index sum exact.
    let high_safe = Integral::from([i32::MAX - 10; 6]);
    let high_safe_rewrite = reducer.rewrite_once(&high_safe).unwrap().unwrap();
    let exact_dot_degree = |integral: &Integral| {
        integral
            .powers()
            .iter()
            .map(|&power| u64::from(power.saturating_sub(1).max(0) as u32))
            .sum::<u64>()
    };
    let target_dots = exact_dot_degree(high_safe_rewrite.target());
    for output in high_safe_rewrite.rhs().terms().keys() {
        if output.denominator_count() == 6 {
            assert_eq!(exact_dot_degree(output) + 1, target_dots);
        }
    }

    assert!(matches!(
        ThreeLoopTopDotReducer::new(
            equal_mass_two_loop_vacuum().unwrap(),
            ThreeLoopTopDotConfig::default(),
        ),
        Err(ThreeLoopTopDotError::WrongLoopCount { actual: 2 })
    ));
}
