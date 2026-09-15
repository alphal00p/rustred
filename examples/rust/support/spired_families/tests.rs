use rustred::algebra::Coefficient;
use rustred::family::ScalarProductCoordinate;
use rustred::solver::SourceSystem;

use super::*;

fn check_linear(family: &IntegralFamily, axis: usize, loop_index: usize, external_index: usize) {
    let c = family.coefficient_context();
    let denominator = &family.denominators()[axis];
    assert_eq!(denominator.constant(), &c.zero());
    for (coordinate, coefficient) in family.coordinates().iter().zip(denominator.coefficients()) {
        let expected = *coordinate
            == ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            };
        assert_eq!(coefficient, &c.integer(i64::from(expected)));
    }
}

/// Expand the stated physical momentum square independently of the literal
/// coefficient matrices in the fixture builder. Only external momentum zero
/// occurs in these quadratic denominators.
fn check_square(
    family: &IntegralFamily,
    axis: usize,
    momentum: &[i64],
    external: i64,
    sign: i64,
    additive: Coefficient,
) {
    let c = family.coefficient_context();
    let denominator = &family.denominators()[axis];
    for (coordinate, coefficient) in family.coordinates().iter().zip(denominator.coefficients()) {
        let integer = match *coordinate {
            ScalarProductCoordinate::LoopLoop { left, right } => {
                sign * momentum[left] * momentum[right] * if left == right { 1 } else { 2 }
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            } => {
                if external_index == 0 {
                    2 * sign * momentum[loop_index] * external
                } else {
                    0
                }
            }
        };
        assert_eq!(coefficient, &c.integer(integer), "D{axis}, {coordinate:?}");
    }
    let constant =
        &additive + &(&c.integer(sign * external * external) * &family.external_gram()[0][0]);
    assert_eq!(denominator.constant(), &constant, "constant of D{axis}");
}

#[test]
fn fam1_11_preserves_the_previously_verified_matrix_and_kinematics() {
    let family = fam1_11().unwrap();
    let c = family.coefficient_context();
    let expected = [
        (0, [0, 0, 0, 0, 1, 0, 0, 0, 0]),
        (0, [0, 0, 0, 0, 0, 0, 0, 1, 0]),
        (0, [0, 0, 0, 0, 0, 1, 0, 0, 0]),
        (0, [0, 0, 0, 0, 0, 0, 0, 0, 1]),
        (0, [-1, 0, 0, 0, 0, 0, 0, 0, 0]),
        (0, [0, 0, -1, 0, 0, 0, 0, 0, 0]),
        (1, [-1, -2, -1, 2, 0, 0, 2, 0, 0]),
        (1, [-1, 0, 0, 2, 0, 0, 0, 0, 0]),
        (1, [0, 0, -1, 0, 0, 0, 2, 0, 0]),
    ];
    for (denominator, (constant, row)) in family.denominators().iter().zip(expected) {
        assert_eq!(denominator.constant(), &c.integer(constant));
        assert_eq!(
            denominator.coefficients(),
            row.map(|v| c.integer(v)).as_slice()
        );
    }
    assert_eq!(family.name(), "spired-fam1-11");
    assert_eq!(family.loop_momenta(), ["k1", "k2"]);
    assert_eq!(family.external_momenta(), ["q", "u1", "u2"]);
    assert_eq!(c.parameter_names(), ["d", "x"]);
    assert_eq!(family.dimension(), &c.parameter("d").unwrap());
    let x = c.parameter("x").unwrap();
    let gamma = &(&(&x * &x) + &c.one()) / &(&c.integer(2) * &x);
    assert_eq!(
        family.external_gram(),
        [
            vec![c.integer(-1), c.zero(), c.zero()],
            vec![c.zero(), c.one(), gamma.clone()],
            vec![c.zero(), gamma, c.one()],
        ]
    );
    assert!(family.power_shifts().iter().all(|shift| shift.is_zero()));
}

#[test]
fn two_loop_pm_variant_changes_only_the_second_velocity_pair() {
    let original = fam1_11().unwrap();
    let variant = fam1_12().unwrap();
    for axis in 0..9 {
        let source_axis = match axis {
            1 => 3,
            3 => 1,
            other => other,
        };
        assert_eq!(
            variant.denominators()[axis],
            original.denominators()[source_axis]
        );
    }
    assert_eq!(original.external_gram(), variant.external_gram());
    assert_eq!(variant.name(), "spired-fam1-12");
    check_linear(&variant, 0, 0, 1);
    check_linear(&variant, 1, 1, 2);
    check_square(
        &variant,
        6,
        &[1, 1],
        -1,
        -1,
        variant.coefficient_context().zero(),
    );
}

#[test]
fn three_loop_pm_families_follow_all_fifteen_ordered_denominators() {
    for (family, third_external) in [(fam1_111().unwrap(), 1), (fam1_112().unwrap(), 2)] {
        let c = family.coefficient_context();
        assert_eq!(family.loop_count(), 3);
        assert_eq!(family.external_count(), 3);
        assert_eq!(family.denominator_count(), 15);
        assert_eq!(c.parameter_names(), ["d", "x"]);
        assert_eq!(family.dimension(), &c.parameter("d").unwrap());
        for loop_index in 0..3 {
            let external = if loop_index == 2 { third_external } else { 1 };
            check_linear(&family, loop_index, loop_index, external);
            check_linear(&family, 3 + loop_index, loop_index, 3 - external);
            let mut momentum = [0; 3];
            momentum[loop_index] = 1;
            check_square(&family, 6 + loop_index, &momentum, 0, -1, c.zero());
            check_square(&family, 9 + loop_index, &momentum, -1, -1, c.zero());
        }
        for (axis, momentum) in [[1, -1, 0], [0, 1, -1], [-1, 0, 1]].iter().enumerate() {
            check_square(&family, 12 + axis, momentum, 0, -1, c.zero());
        }
        assert_eq!(family.external_gram(), fam1_11().unwrap().external_gram());
        assert!(family.power_shifts().iter().all(|shift| shift.is_zero()));
    }
}

#[test]
fn four_loop_h_uses_the_fmft_parent_momentum_basis() {
    let family = four_loop_h().unwrap();
    let c = family.coefficient_context();
    assert_eq!(family.name(), "spired-four-loop-unit-mass-vacuum-h");
    assert_eq!(family.loop_count(), 4);
    assert_eq!(family.external_count(), 0);
    assert_eq!(family.denominator_count(), 10);
    assert_eq!(family.coordinates().len(), 10);
    assert_eq!(c.parameter_names(), ["d"]);
    assert_eq!(family.dimension(), &c.parameter("d").unwrap());
    let expected = [
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [1, 0, -2, 0, 0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 1, -2, 0, 1, 0, 0],
        [1, 0, -2, -2, 0, 0, 0, 1, 2, 1],
        [0, 0, 0, 0, 1, -2, -2, 1, 2, 1],
        [0, 0, 0, 0, 0, 0, 0, 1, 2, 1],
        [1, -2, 0, 0, 1, 0, 0, 0, 0, 0],
    ];
    for (denominator, row) in family.denominators().iter().zip(expected) {
        assert_eq!(denominator.constant(), &c.integer(-1));
        assert_eq!(
            denominator.coefficients(),
            row.map(|value| c.integer(value)).as_slice()
        );
    }
    let sources = SourceSystem::<10>::from_family(&family).unwrap();
    assert_eq!(sources.rows().len(), 16);
    assert!(family.power_shifts().iter().all(|shift| shift.is_zero()));
}

#[test]
fn bc4pm_rad1_retains_its_noninteger_offset_and_euclidean_gram() {
    let family = bc4pm_rad1().unwrap();
    let c = family.coefficient_context();
    assert_eq!(family.external_momenta(), ["q", "n"]);
    assert_eq!(c.parameter_names(), ["d", "ep2"]);
    assert_eq!(family.dimension(), &c.parameter("d").unwrap());
    assert_eq!(
        family.external_gram(),
        [vec![c.one(), c.zero()], vec![c.zero(), c.one()]]
    );
    assert_eq!(family.power_shifts()[0], c.parameter("ep2").unwrap());
    assert!(
        family.power_shifts()[1..]
            .iter()
            .all(|shift| shift.is_zero())
    );
    check_linear(&family, 0, 0, 1);
    check_linear(&family, 1, 1, 1);
    for (axis, momentum, external) in [
        (2, [1, 0], 0),
        (3, [0, 1], 0),
        (4, [1, 0], -1),
        (5, [0, 1], -1),
        (6, [1, -1], 0),
    ] {
        check_square(&family, axis, &momentum, external, 1, c.zero());
    }
    let sources = SourceSystem::<7>::from_family(&family).unwrap();
    assert_eq!(sources.rows().len(), 9);
    assert_eq!(sources.fixed(), &[None; 7]);
    assert!(
        sources
            .rows()
            .iter()
            .flatten()
            .any(|term| term.coefficient.contains(1)),
        "the ep2 offset must survive ordinary/LI source generation"
    );
}

#[test]
fn cosmo_keeps_all_masses_and_the_external_square_independent() {
    let family = fam_cosmo().unwrap();
    let c = family.coefficient_context();
    assert_eq!(family.external_momenta(), ["p"]);
    assert_eq!(c.parameter_names(), ["d", "M1", "M2", "M3", "s"]);
    assert_eq!(family.dimension(), &c.parameter("d").unwrap());
    assert_eq!(family.external_gram(), [vec![c.parameter("s").unwrap()]]);
    assert!(family.power_shifts().iter().all(|shift| shift.is_zero()));
    for (axis, momentum, external, mass) in [
        (0, [1, 0], 0, c.parameter("M1").unwrap()),
        (1, [1, 0], 1, c.zero()),
        (2, [0, 1], 1, c.parameter("M2").unwrap()),
        (3, [0, 1], 0, c.zero()),
        (4, [1, -1], 0, c.parameter("M3").unwrap()),
    ] {
        check_square(&family, axis, &momentum, external, 1, mass);
    }
    let sources = SourceSystem::<5>::from_family(&family).unwrap();
    assert_eq!(sources.rows().len(), 6);
    assert!(
        sources
            .rows()
            .iter()
            .flatten()
            .any(|term| term.coefficient.contains(4)),
        "the independent p² invariant must survive source generation"
    );
}

#[test]
fn supplied_pm_families_generate_the_reference_ordinary_plus_li_census() {
    for family in [fam1_11().unwrap(), fam1_12().unwrap()] {
        let sources = SourceSystem::<9>::from_family(&family).unwrap();
        assert_eq!(sources.rows().len(), 13);
        assert_eq!(sources.fixed(), &[None; 9]);
        assert_eq!(
            SourceSystem::<9>::from_family_with_lorentz(&family, false)
                .unwrap()
                .rows()
                .len(),
            10
        );
    }
    for family in [fam1_111().unwrap(), fam1_112().unwrap()] {
        let sources = SourceSystem::<15>::from_family(&family).unwrap();
        assert_eq!(sources.rows().len(), 21);
        assert_eq!(sources.fixed(), &[None; 15]);
        assert_eq!(
            SourceSystem::<15>::from_family_with_lorentz(&family, false)
                .unwrap()
                .rows()
                .len(),
            18
        );
    }
}
