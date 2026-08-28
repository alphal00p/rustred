use std::cmp::Ordering;

use rustred_legacy_oracles::Integral;
use rustred_legacy_oracles::families::equal_mass_two_loop_vacuum;
use rustred_legacy_oracles::{
    THREE_LOOP_F5_CENTRAL_IBP_WEIGHTS, THREE_LOOP_F5_OUTER_IBP_WEIGHTS,
    THREE_LOOP_PROPER_DOT_RAW_TERM_BOUND, ThreeLoopProperDotConfig, ThreeLoopProperDotError,
    ThreeLoopProperDotReducer, ThreeLoopProperDotSector,
};

fn exact_dot_degree(integral: &Integral) -> u64 {
    integral
        .powers()
        .iter()
        .map(|&power| u64::from(power.saturating_sub(1).max(0) as u32))
        .sum()
}

fn check_f5_box(reducer: &ThreeLoopProperDotReducer) {
    for encoded in 0_u32..3_u32.pow(5) {
        let mut powers = [1, 1, 1, 1, 1, 0];
        let mut value = encoded;
        for power in &mut powers[..5] {
            *power = i32::try_from(value % 3 + 1).unwrap();
            value /= 3;
        }
        let input = Integral::from(powers);
        let Some(rewrite) = reducer.rewrite_once(&input).unwrap() else {
            assert_eq!(input, Integral::from([1, 1, 1, 1, 1, 0]));
            continue;
        };
        assert_eq!(rewrite.sector(), ThreeLoopProperDotSector::F5);
        let pivot = rewrite.provenance().seed_lowered_position();
        assert!(rewrite.target().powers()[pivot] > 1);
        assert_eq!(
            rewrite.seed().powers()[pivot] + 1,
            rewrite.target().powers()[pivot]
        );
        assert!(matches!(
            rewrite.provenance().raw_ibp_weights(),
            THREE_LOOP_F5_CENTRAL_IBP_WEIGHTS | THREE_LOOP_F5_OUTER_IBP_WEIGHTS
        ));
        assert!(rewrite.rhs().terms().keys().all(|output| {
            output.numerator_degree() == 0
                && reducer.family().compare_integrals(output, rewrite.target()) == Ordering::Less
        }));
        for output in rewrite.rhs().terms().keys() {
            if output.denominator_count() == 5 {
                assert!(exact_dot_degree(output) < exact_dot_degree(rewrite.target()));
            }
        }
    }
}

// Restricted Symbolica must stay on one worker, so all provenance, exhaustive
// boxes, high-index arithmetic, and typed guard checks share one test.
#[test]
fn certified_three_loop_genuine_proper_dot_recurrences() {
    let reducer = ThreeLoopProperDotReducer::build(ThreeLoopProperDotConfig::default()).unwrap();
    assert_eq!(THREE_LOOP_PROPER_DOT_RAW_TERM_BOUND, 41);

    // F5 has two edge orbits under its stabilizer. These asymmetric points
    // force both independently weighted native-row combinations.
    for target in [
        Integral::from([4, 2, 1, 3, 2, 0]),
        Integral::from([1, 4, 2, 3, 2, 0]),
        Integral::from([2, 3, 1, 4, 2, 0]),
    ] {
        reducer.validate_raw_ibp_provenance(&target).unwrap();
        assert_eq!(
            reducer.raw_ibp(&target).unwrap(),
            reducer.expected_raw_ibp(&target).unwrap()
        );
    }
    let central = reducer
        .rewrite_once(&Integral::from([4, 2, 1, 3, 2, 0]))
        .unwrap()
        .unwrap();
    let outer = reducer
        .rewrite_once(&Integral::from([1, 4, 2, 3, 2, 0]))
        .unwrap()
        .unwrap();
    assert_eq!(central.provenance().seed_lowered_position(), 0);
    assert_eq!(
        central.provenance().raw_ibp_weights(),
        THREE_LOOP_F5_CENTRAL_IBP_WEIGHTS
    );
    assert_eq!(outer.provenance().seed_lowered_position(), 1);
    assert_eq!(
        outer.provenance().raw_ibp_weights(),
        THREE_LOOP_F5_OUTER_IBP_WEIGHTS
    );

    check_f5_box(&reducer);
    assert_eq!(
        reducer
            .rewrite_once(&Integral::from([1, 1, 1, 1, 1, 0]))
            .unwrap(),
        None
    );
    assert_eq!(
        reducer
            .rewrite_once(&Integral::from([1, 1, 0, 1, 0, 1]))
            .unwrap(),
        None
    );
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([3, 2, 0, 4, 0, 5])),
        Err(ThreeLoopProperDotError::UnsupportedDottedB4 { .. })
    ));

    let capped =
        ThreeLoopProperDotReducer::build(ThreeLoopProperDotConfig { max_raw_terms: 40 }).unwrap();
    assert!(matches!(
        capped.rewrite_once(&Integral::from([2, 1, 1, 1, 1, 0])),
        Err(ThreeLoopProperDotError::ResourceLimit {
            resource: "raw identity terms",
            requested: 41,
            limit: 40,
        })
    ));
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([2, 1, 1, 1, 1])),
        Err(ThreeLoopProperDotError::WrongIntegralArity {
            expected: 6,
            actual: 5
        })
    ));
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([2, 1, 1, 1, 1, 1])),
        Err(ThreeLoopProperDotError::OutsideGenuineProperSector { .. })
    ));
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([2, 1, 1, 1, 1, -1])),
        Err(ThreeLoopProperDotError::UnexpectedNumeratorInput {
            position: 5,
            power: -1,
            ..
        })
    ));
    assert!(matches!(
        reducer.validate_raw_ibp_provenance(&Integral::from([1, 1, 1, 1, 1, 0])),
        Err(ThreeLoopProperDotError::PivotGuardNotSatisfied { .. })
    ));

    // Domain rejection is checked before the native rows or Symbolica division.
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([i32::MIN, 1, 1, 1, 1, 0])),
        Err(ThreeLoopProperDotError::UnexpectedNumeratorInput { .. })
    ));

    // The outer row combination includes a shift which raises a nonpivot
    // outer line; at MAX it overflows in the coefficient-free preflight.
    assert!(matches!(
        reducer.rewrite_once(&Integral::from([
            1,
            i32::MAX,
            i32::MAX,
            i32::MAX,
            i32::MAX,
            0
        ])),
        Err(ThreeLoopProperDotError::ExponentOverflow { .. })
    ));

    // Every native derivative shift is safe at these high powers, and exact
    // unsaturated degrees still prove strict descent.
    for high in [
        Integral::from([
            i32::MAX - 1,
            i32::MAX - 2,
            i32::MAX - 3,
            i32::MAX - 4,
            i32::MAX - 5,
            0,
        ]),
        Integral::from([1, i32::MAX - 1, i32::MAX - 2, i32::MAX - 3, i32::MAX - 4, 0]),
    ] {
        let rewrite = reducer.rewrite_once(&high).unwrap().unwrap();
        assert!(
            rewrite
                .rhs()
                .terms()
                .keys()
                .all(|output| output.denominator_count() < 5
                    || exact_dot_degree(output) < exact_dot_degree(rewrite.target()))
        );
    }

    assert!(matches!(
        ThreeLoopProperDotReducer::new(
            equal_mass_two_loop_vacuum().unwrap(),
            ThreeLoopProperDotConfig::default()
        ),
        Err(ThreeLoopProperDotError::WrongLoopCount { actual: 2 })
    ));
}
