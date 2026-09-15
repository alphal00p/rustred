//! Independent adversarial tests for the necessary row-bound proof.

use super::*;

#[test]
fn constructed_integer_witnesses_are_never_excluded() {
    // Every RHS is constructed from an actual point in the stated sector.
    // Sampling here is only adversarial test coverage, never proof authority
    // in the implementation. Include every coefficient/sector sign and every
    // fixed-axis subset, then reverse the complete equation's sign.
    for a in -2_i16..=2 {
        for b in -2_i16..=2 {
            for c in -2_i16..=2 {
                let coefficients = [Integer::from(a), Integer::from(b), Integer::from(c)];
                for sector_mask in 0_u8..8 {
                    let sector =
                        std::array::from_fn::<_, 3, _>(|axis| sector_mask & (1 << axis) != 0);
                    for witness_mask in 0_u8..8 {
                        let witness = std::array::from_fn::<_, 3, _>(|axis| {
                            let far = witness_mask & (1 << axis) != 0;
                            match (sector[axis], far) {
                                (true, false) => 1,
                                (true, true) => 4,
                                (false, false) => 0,
                                (false, true) => -3,
                            }
                        });
                        let rhs = coefficients
                            .iter()
                            .zip(witness)
                            .fold(Integer::zero(), |sum, (coefficient, value)| {
                                sum + coefficient * Integer::from(value)
                            });
                        let row = [
                            coefficients[0].clone(),
                            coefficients[1].clone(),
                            coefficients[2].clone(),
                            rhs,
                        ];
                        let negated = row.each_ref().map(|entry| -entry);
                        for fixed_mask in 0_u8..8 {
                            let face = CoordinateCase::new(std::array::from_fn(|axis| {
                                (fixed_mask & (1 << axis) != 0).then_some(witness[axis])
                            }))
                            .unwrap();
                            assert!(!excludes_rhs(&row, &face, &sector));
                            assert!(!excludes_rhs(&negated, &face, &sector));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn zero_coefficients_do_not_make_finite_bounds_unbounded() {
    let face = CoordinateCase::<2>::generic();
    for sector in [[false, true], [true, true]] {
        assert!(excludes_rhs(
            &[Integer::zero(), Integer::one(), Integer::zero()],
            &face,
            &sector,
        ));
        assert!(!excludes_rhs(
            &[Integer::zero(), Integer::one(), Integer::one()],
            &face,
            &sector,
        ));
    }
    for sector in [[false; 2], [true; 2], [false, true], [true, false]] {
        for rhs in [-1, 0, 1] {
            assert_eq!(
                excludes_rhs(
                    &[Integer::zero(), Integer::zero(), Integer::from(rhs)],
                    &face,
                    &sector,
                ),
                rhs != 0,
            );
        }
    }
}

#[test]
fn large_fixed_contributions_and_exact_boundary_survive_row_negation() {
    let big = Integer::from(10).pow(80) + Integer::from(7);
    let face = CoordinateCase::new([Some(-3), Some(2), None]).unwrap();
    let sector = [false, true, true];
    // The fixed contribution is -3*big + 2*(2*big) = big. The remaining
    // active coordinate contributes at least 5, with no finite upper bound.
    for (rhs, expected) in [
        (&big + Integer::from(4), true),
        (&big + Integer::from(5), false),
        (big.pow(2), false),
    ] {
        let row = [big.clone(), &big * Integer::from(2), Integer::from(5), rhs];
        let negated = row.each_ref().map(|entry| -entry);
        assert_eq!(excludes_rhs(&row, &face, &sector), expected);
        assert_eq!(excludes_rhs(&negated, &face, &sector), expected);
    }
    let fixed_face = CoordinateCase::new([Some(-3), Some(2), Some(1)]).unwrap();
    for delta in [-1, 0, 1] {
        let row = [
            big.clone(),
            &big * Integer::from(2),
            Integer::from(5),
            &big + Integer::from(5 + delta),
        ];
        assert_eq!(excludes_rhs(&row, &fixed_face, &sector), delta != 0);
    }
}
