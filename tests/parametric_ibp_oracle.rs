//! Black-box validation of the generic parametric IBP generator.
//!
//! The oracle in this target intentionally does not call
//! `IntegralFamily::derivative_contraction`, `scalar_product_expansion`, or
//! `inverse_basis`, and it does not reproduce `ParametricIbpGenerator`'s
//! parametric relation-building code.  Instead it:
//!
//! 1. inverts the input denominator matrix with a tiny test-local exact
//!    Gauss--Jordan solver;
//! 2. differentiates the public scalar-product coordinates directly;
//! 3. builds concrete IBP equations at integer powers; and
//! 4. builds LI equations from independently generated concrete external
//!    rows evaluated at the shifted powers required by denominator
//!    multiplication.
//!
//! This makes the production generator and the validation path share only the
//! mathematical family definition and Symbolica's exact coefficient field.

use std::collections::BTreeMap;

use rustred::{
    AffineDenominator, Coefficient, CoefficientContext, ContractionMomentum, IntegralFamily,
    ParametricArithmeticLimits, ParametricIbpGenerator, ParametricRowId, ScalarProductCoordinate,
};

type OracleRelation = BTreeMap<Vec<i64>, Coefficient>;

fn add_coefficient(context: &CoefficientContext, target: &mut Coefficient, value: &Coefficient) {
    *target = &*target + value;
    assert!(context.contains(target));
}

fn add_term(
    context: &CoefficientContext,
    relation: &mut OracleRelation,
    powers: Vec<i64>,
    coefficient: Coefficient,
) {
    assert!(context.contains(&coefficient));
    if coefficient.is_zero() {
        return;
    }
    if let Some(current) = relation.get(&powers) {
        let sum = current + &coefficient;
        if sum.is_zero() {
            relation.remove(&powers);
        } else {
            assert!(context.contains(&sum));
            relation.insert(powers, sum);
        }
    } else {
        relation.insert(powers, coefficient);
    }
}

fn add_scaled_relation(
    context: &CoefficientContext,
    target: &mut OracleRelation,
    source: &OracleRelation,
    factor: &Coefficient,
) {
    for (powers, coefficient) in source {
        add_term(context, target, powers.clone(), coefficient * factor);
    }
}

/// Test-local exact matrix inversion.  This only sees the public denominator
/// rows and deliberately does not inspect the inverse cached by RustRed.
fn invert_denominator_matrix(family: &IntegralFamily) -> Vec<Vec<Coefficient>> {
    let context = family.coefficient_context();
    let size = family.denominator_count();
    let mut augmented = Vec::with_capacity(size);
    for row in 0..size {
        let mut values = Vec::with_capacity(size * 2);
        values.extend(family.denominators()[row].coefficients().iter().cloned());
        values.extend((0..size).map(|column| {
            if row == column {
                context.one()
            } else {
                context.zero()
            }
        }));
        augmented.push(values);
    }

    for pivot_column in 0..size {
        let pivot_row = (pivot_column..size)
            .find(|&row| !augmented[row][pivot_column].is_zero())
            .expect("test family denominator matrix must be invertible");
        augmented.swap(pivot_column, pivot_row);

        let pivot = augmented[pivot_column][pivot_column].clone();
        for entry in &mut augmented[pivot_column] {
            *entry = &*entry / &pivot;
            assert!(context.contains(entry));
        }

        let normalized_pivot_row = augmented[pivot_column].clone();
        for row in 0..size {
            if row == pivot_column {
                continue;
            }
            let factor = augmented[row][pivot_column].clone();
            if factor.is_zero() {
                continue;
            }
            for column in 0..size * 2 {
                let subtraction = &factor * &normalized_pivot_row[column];
                augmented[row][column] = &augmented[row][column] - &subtraction;
                assert!(context.contains(&augmented[row][column]));
            }
        }
    }

    // Verify both products.  Besides checking the tiny solver, this fixes the
    // orientation used below: S_s = sum_t inverse[s][t] (D_t-c_t).
    let inverse = augmented
        .into_iter()
        .map(|row| row.into_iter().skip(size).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for left in 0..size {
        for right in 0..size {
            let mut product = context.zero();
            for middle in 0..size {
                let contribution =
                    &family.denominators()[left].coefficients()[middle] * &inverse[middle][right];
                add_coefficient(context, &mut product, &contribution);
            }
            let expected = if left == right {
                context.one()
            } else {
                context.zero()
            };
            assert!((&product - &expected).is_zero());
        }
    }
    inverse
}

fn coordinate_position(family: &IntegralFamily, wanted: ScalarProductCoordinate) -> usize {
    family
        .coordinates()
        .iter()
        .position(|&candidate| candidate == wanted)
        .expect("coordinate is present in the complete family")
}

fn loop_loop_position(family: &IntegralFamily, left: usize, right: usize) -> usize {
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    coordinate_position(family, ScalarProductCoordinate::LoopLoop { left, right })
}

fn loop_external_position(
    family: &IntegralFamily,
    loop_index: usize,
    external_index: usize,
) -> usize {
    coordinate_position(
        family,
        ScalarProductCoordinate::LoopExternal {
            loop_index,
            external_index,
        },
    )
}

fn add_dot_with_loop(
    family: &IntegralFamily,
    scalar_coefficients: &mut [Coefficient],
    contraction: ContractionMomentum,
    loop_index: usize,
    coefficient: &Coefficient,
) {
    let coordinate = match contraction {
        ContractionMomentum::Loop(other_loop) => loop_loop_position(family, loop_index, other_loop),
        ContractionMomentum::External(external_index) => {
            loop_external_position(family, loop_index, external_index)
        }
    };
    add_coefficient(
        family.coefficient_context(),
        &mut scalar_coefficients[coordinate],
        coefficient,
    );
}

fn add_dot_with_external(
    family: &IntegralFamily,
    constant: &mut Coefficient,
    scalar_coefficients: &mut [Coefficient],
    contraction: ContractionMomentum,
    external_index: usize,
    coefficient: &Coefficient,
) {
    match contraction {
        ContractionMomentum::Loop(loop_index) => {
            let coordinate = loop_external_position(family, loop_index, external_index);
            add_coefficient(
                family.coefficient_context(),
                &mut scalar_coefficients[coordinate],
                coefficient,
            );
        }
        ContractionMomentum::External(other_external) => {
            let contribution =
                coefficient * &family.external_gram()[other_external][external_index];
            add_coefficient(family.coefficient_context(), constant, &contribution);
        }
    }
}

/// Differentiate an input denominator as a free affine form in the declared
/// scalar products.  No production derivative cache is consulted.
fn differentiate_denominator(
    family: &IntegralFamily,
    denominator: usize,
    differentiated_loop: usize,
    contraction: ContractionMomentum,
) -> (Coefficient, Vec<Coefficient>) {
    let context = family.coefficient_context();
    let mut constant = context.zero();
    let mut scalar_coefficients = vec![context.zero(); family.denominator_count()];

    for (coordinate, scalar_product) in family.coordinates().iter().copied().enumerate() {
        let coefficient = &family.denominators()[denominator].coefficients()[coordinate];
        if coefficient.is_zero() {
            continue;
        }
        match scalar_product {
            ScalarProductCoordinate::LoopLoop { left, right } => {
                if differentiated_loop == left {
                    add_dot_with_loop(
                        family,
                        &mut scalar_coefficients,
                        contraction,
                        right,
                        coefficient,
                    );
                }
                if differentiated_loop == right {
                    add_dot_with_loop(
                        family,
                        &mut scalar_coefficients,
                        contraction,
                        left,
                        coefficient,
                    );
                }
            }
            ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            } => {
                if differentiated_loop == loop_index {
                    add_dot_with_external(
                        family,
                        &mut constant,
                        &mut scalar_coefficients,
                        contraction,
                        external_index,
                        coefficient,
                    );
                }
            }
        }
    }
    (constant, scalar_coefficients)
}

/// Rewrite `constant + sum_s scalar[s] S_s` in the denominator basis using
/// the independently solved inverse.
fn rewrite_in_denominators(
    family: &IntegralFamily,
    inverse: &[Vec<Coefficient>],
    direct_constant: Coefficient,
    scalar_coefficients: &[Coefficient],
) -> (Coefficient, Vec<Coefficient>) {
    let context = family.coefficient_context();
    let size = family.denominator_count();
    let mut denominator_coefficients = vec![context.zero(); size];
    for scalar in 0..size {
        for target in 0..size {
            let contribution = &scalar_coefficients[scalar] * &inverse[scalar][target];
            add_coefficient(
                context,
                &mut denominator_coefficients[target],
                &contribution,
            );
        }
    }
    let mut constant = direct_constant;
    for target in 0..size {
        let subtraction =
            &denominator_coefficients[target] * family.denominators()[target].constant();
        constant = &constant - &subtraction;
    }
    (constant, denominator_coefficients)
}

fn contraction_at(family: &IntegralFamily, position: usize) -> ContractionMomentum {
    if position < family.loop_count() {
        ContractionMomentum::Loop(position)
    } else {
        ContractionMomentum::External(position - family.loop_count())
    }
}

/// Direct concrete form of one ordinary IBP row.
fn ordinary_oracle(
    family: &IntegralFamily,
    inverse: &[Vec<Coefficient>],
    powers: &[i64],
    contraction_position: usize,
    differentiated_loop: usize,
) -> OracleRelation {
    let context = family.coefficient_context();
    let contraction = contraction_at(family, contraction_position);
    let mut relation = OracleRelation::new();
    if contraction == ContractionMomentum::Loop(differentiated_loop) {
        add_term(
            context,
            &mut relation,
            powers.to_vec(),
            family.dimension().clone(),
        );
    }

    for denominator in 0..family.denominator_count() {
        let exponent = &context.integer(powers[denominator]) + &family.power_shifts()[denominator];
        let (direct_constant, direct_scalars) =
            differentiate_denominator(family, denominator, differentiated_loop, contraction);
        let (constant, denominator_coefficients) =
            rewrite_in_denominators(family, inverse, direct_constant, &direct_scalars);

        let mut raised = powers.to_vec();
        raised[denominator] = raised[denominator]
            .checked_add(1)
            .expect("small oracle power does not overflow");
        add_term(
            context,
            &mut relation,
            raised.clone(),
            -(&exponent * &constant),
        );
        for target in 0..family.denominator_count() {
            let mut shifted = raised.clone();
            shifted[target] = shifted[target]
                .checked_sub(1)
                .expect("small oracle power does not overflow");
            add_term(
                context,
                &mut relation,
                shifted,
                -(&exponent * &denominator_coefficients[target]),
            );
        }
    }
    relation
}

/// Independently compose a concrete LI row.  Multiplication by
/// `k_i.p_a = beta_0 + sum_t beta_t D_t` is implemented by evaluating the
/// direct ordinary oracle at `n-e_t`, rather than by translating a generated
/// parametric relation.
fn li_oracle(
    family: &IntegralFamily,
    inverse: &[Vec<Coefficient>],
    powers: &[i64],
    first_external: usize,
    second_external: usize,
) -> OracleRelation {
    let context = family.coefficient_context();
    let mut relation = OracleRelation::new();
    for differentiated_loop in 0..family.loop_count() {
        // M_ba = X_{i b} B_{a i}
        add_weighted_external_oracle(
            family,
            inverse,
            powers,
            first_external,
            differentiated_loop,
            second_external,
            &context.one(),
            &mut relation,
        );
        // -M_ab = -X_{i a} B_{b i}
        add_weighted_external_oracle(
            family,
            inverse,
            powers,
            second_external,
            differentiated_loop,
            first_external,
            &context.integer(-1),
            &mut relation,
        );
    }
    relation
}

#[allow(clippy::too_many_arguments)]
fn add_weighted_external_oracle(
    family: &IntegralFamily,
    inverse: &[Vec<Coefficient>],
    powers: &[i64],
    ordinary_external: usize,
    differentiated_loop: usize,
    multiplier_external: usize,
    overall_sign: &Coefficient,
    target: &mut OracleRelation,
) {
    let context = family.coefficient_context();
    let coordinate = loop_external_position(family, differentiated_loop, multiplier_external);
    let denominator_coefficients = &inverse[coordinate];
    let mut constant = context.zero();
    for denominator in 0..family.denominator_count() {
        let subtraction =
            &denominator_coefficients[denominator] * family.denominators()[denominator].constant();
        constant = &constant - &subtraction;
    }

    if !constant.is_zero() {
        let ordinary = ordinary_oracle(
            family,
            inverse,
            powers,
            family.loop_count() + ordinary_external,
            differentiated_loop,
        );
        add_scaled_relation(context, target, &ordinary, &(&constant * overall_sign));
    }
    for denominator in 0..family.denominator_count() {
        let coefficient = &denominator_coefficients[denominator];
        if coefficient.is_zero() {
            continue;
        }
        let mut translated_powers = powers.to_vec();
        translated_powers[denominator] = translated_powers[denominator]
            .checked_sub(1)
            .expect("small oracle power does not overflow");
        let ordinary = ordinary_oracle(
            family,
            inverse,
            &translated_powers,
            family.loop_count() + ordinary_external,
            differentiated_loop,
        );
        add_scaled_relation(context, target, &ordinary, &(coefficient * overall_sign));
    }
}

fn assert_relation_eq(
    family: &IntegralFamily,
    label: &str,
    actual: &rustred::ConcreteRelation,
    expected: &OracleRelation,
) {
    let actual_keys = actual
        .terms()
        .keys()
        .map(|key| key.powers().to_vec())
        .collect::<Vec<_>>();
    let expected_keys = expected.keys().cloned().collect::<Vec<_>>();
    assert_eq!(actual_keys, expected_keys, "{label}: integral keys differ");
    for (powers, expected_coefficient) in expected {
        let actual_coefficient = actual
            .terms()
            .iter()
            .find_map(|(key, coefficient)| (key.powers() == powers).then_some(coefficient))
            .expect("key equality was checked above");
        assert!(
            (actual_coefficient - expected_coefficient).is_zero(),
            "{label}, powers {powers:?}: actual={actual_coefficient}, expected={expected_coefficient}"
        );
        assert!(family.coefficient_context().contains(actual_coefficient));
    }
}

fn assert_family_guards_survive(
    family: &IntegralFamily,
    concrete: &rustred::ConcreteRelation,
    label: &str,
) {
    for expected in family
        .domain()
        .conditions()
        .filter(|condition| !condition.polynomial().is_constant())
    {
        assert!(
            concrete
                .nonzero_conditions()
                .iter()
                .any(|actual| actual.raw() == expected.polynomial()),
            "{label}: missing family-domain guard from {:?}: {}",
            expected.source(),
            expected.polynomial()
        );
    }
}

fn validate_all_rows(family: &IntegralFamily, assignments: &[Vec<i64>]) {
    let inverse = invert_denominator_matrix(family);
    let generated = ParametricIbpGenerator::try_new(family)
        .expect("generator construction")
        .generate()
        .expect("parametric generation");
    let limits = ParametricArithmeticLimits::default();
    let expected_ordinary_count =
        family.loop_count() * (family.loop_count() + family.external_count());
    assert_eq!(generated.ordinary_ibp().len(), expected_ordinary_count);
    assert_eq!(
        generated.lorentz_invariance().len(),
        family.external_count() * family.external_count().saturating_sub(1) / 2
    );

    for powers in assignments {
        assert_eq!(powers.len(), family.denominator_count());
        for contraction_position in 0..family.loop_count() + family.external_count() {
            for differentiated_loop in 0..family.loop_count() {
                let row_position = contraction_position * family.loop_count() + differentiated_loop;
                let row = &generated.ordinary_ibp()[row_position];
                assert_eq!(
                    row.row_id(),
                    &ParametricRowId::OrdinaryIbp {
                        contraction_momentum: contraction_position,
                        differentiated_loop,
                    }
                );
                let actual = row
                    .specialize(generated.context(), powers, limits)
                    .expect("ordinary row specialization");
                let expected = ordinary_oracle(
                    family,
                    &inverse,
                    powers,
                    contraction_position,
                    differentiated_loop,
                );
                let label = format!(
                    "{} ordinary q={contraction_position}, i={differentiated_loop}, n={powers:?}",
                    family.name()
                );
                assert_relation_eq(family, &label, &actual, &expected);
                assert_family_guards_survive(family, &actual, &label);
            }
        }

        let mut li_position = 0;
        for first_external in 0..family.external_count() {
            for second_external in first_external + 1..family.external_count() {
                let row = &generated.lorentz_invariance()[li_position];
                assert_eq!(
                    row.row_id(),
                    &ParametricRowId::LorentzInvariance {
                        first_external,
                        second_external,
                    }
                );
                let actual = row
                    .specialize(generated.context(), powers, limits)
                    .expect("LI row specialization");
                let expected = li_oracle(family, &inverse, powers, first_external, second_external);
                let label = format!(
                    "{} LI ({first_external},{second_external}), n={powers:?}",
                    family.name()
                );
                assert_relation_eq(family, &label, &actual, &expected);
                assert_family_guards_survive(family, &actual, &label);
                li_position += 1;
            }
        }
    }
}

fn affine(
    constant: Coefficient,
    coefficients: impl IntoIterator<Item = Coefficient>,
) -> AffineDenominator {
    AffineDenominator::new(constant, coefficients.into_iter().collect())
}

fn vacuum_coordinate_position(loops: usize, left: usize, right: usize) -> usize {
    let wanted = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    let mut position = 0;
    for first in 0..loops {
        for second in first..loops {
            if (first, second) == wanted {
                return position;
            }
            position += 1;
        }
    }
    panic!("vacuum coordinate is outside the loop basis")
}

#[test]
fn oracle_one_loop_vacuum_affine_rational_basis_and_power_shift() {
    let context = CoefficientContext::new(["d", "a", "h", "m", "nu"]);
    let a_over_h = context.parse("a/h").unwrap();
    let family = IntegralFamily::new(
        "oracle-1l-e0-rational",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![affine(context.parameter("m").unwrap(), [a_over_h])],
        Vec::new(),
        vec![context.parameter("nu").unwrap()],
    )
    .unwrap();

    // Includes n=0: the nonzero PowerShift contribution must not disappear.
    validate_all_rows(&family, &[vec![-2], vec![0], vec![1], vec![4]]);
}

#[test]
fn oracle_one_loop_one_external_nonsymmetric_symbolic_basis() {
    let context = CoefficientContext::new(["d", "a", "b", "h", "m0", "m1", "g", "nu0", "nu1"]);
    let family = IntegralFamily::new(
        "oracle-1l-e1-nonsymmetric",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            affine(
                context.parameter("m0").unwrap(),
                [context.parse("a/h").unwrap(), context.integer(2)],
            ),
            affine(
                context.parameter("m1").unwrap(),
                [context.integer(3), context.parameter("b").unwrap()],
            ),
        ],
        vec![vec![context.parameter("g").unwrap()]],
        vec![
            context.parameter("nu0").unwrap(),
            context.parameter("nu1").unwrap(),
        ],
    )
    .unwrap();
    assert!(!family.domain().input_denominators().is_empty());
    assert!(
        !family
            .domain()
            .determinant_nonzero()
            .polynomial()
            .is_constant()
    );
    validate_all_rows(&family, &[vec![0, 0], vec![2, -1], vec![-2, 3], vec![4, 5]]);
}

#[test]
fn oracle_one_loop_two_external_affine_rows_and_li() {
    let context = CoefficientContext::new([
        "d", "m0", "m1", "m2", "s00", "s01", "s11", "nu0", "nu1", "nu2",
    ]);
    let family = IntegralFamily::new(
        "oracle-1l-e2-li",
        vec!["k".into()],
        vec!["p0".into(), "p1".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            affine(
                context.parameter("m0").unwrap(),
                [context.integer(1), context.integer(2), context.integer(0)],
            ),
            affine(
                context.parameter("m1").unwrap(),
                [context.integer(0), context.integer(1), context.integer(3)],
            ),
            affine(
                context.parameter("m2").unwrap(),
                [context.integer(2), context.integer(0), context.integer(1)],
            ),
        ],
        vec![
            vec![
                context.parameter("s00").unwrap(),
                context.parameter("s01").unwrap(),
            ],
            vec![
                context.parameter("s01").unwrap(),
                context.parameter("s11").unwrap(),
            ],
        ],
        vec![
            context.parameter("nu0").unwrap(),
            context.parameter("nu1").unwrap(),
            context.parameter("nu2").unwrap(),
        ],
    )
    .unwrap();

    validate_all_rows(
        &family,
        &[vec![0, 1, 2], vec![3, -1, 2], vec![-2, 0, 4], vec![2, 3, 4]],
    );
}

#[test]
fn oracle_two_loop_vacuum_all_contractions_and_derivatives() {
    let context = CoefficientContext::new(["d", "m0", "m1", "m2", "nu0", "nu1", "nu2"]);
    let family = IntegralFamily::new(
        "oracle-2l-e0-nonsymmetric",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            affine(
                context.parameter("m0").unwrap(),
                [context.integer(1), context.integer(2), context.integer(0)],
            ),
            affine(
                context.parameter("m1").unwrap(),
                [context.integer(0), context.integer(1), context.integer(1)],
            ),
            affine(
                context.parameter("m2").unwrap(),
                [context.integer(2), context.integer(0), context.integer(1)],
            ),
        ],
        Vec::new(),
        vec![
            context.parameter("nu0").unwrap(),
            context.parameter("nu1").unwrap(),
            context.parameter("nu2").unwrap(),
        ],
    )
    .unwrap();

    validate_all_rows(
        &family,
        &[vec![0, 0, 0], vec![1, 2, 3], vec![-1, 3, 0], vec![4, -2, 2]],
    );
}

#[test]
fn oracle_two_loop_one_external_complete_affine_family() {
    let context = CoefficientContext::new(["d", "m2", "s", "nu0", "nu1", "nu2", "nu3", "nu4"]);
    let zero = context.zero();
    let one = context.one();
    let two = context.integer(2);
    let minus_m2 = context.parse("-m2").unwrap();
    let family = IntegralFamily::new(
        "oracle-2l-e1-complete-affine",
        vec!["k0".into(), "k1".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        // Coordinate order is k0^2,k0.k1,k1^2,k0.p,k1.p.
        vec![
            affine(
                minus_m2.clone(),
                [
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                minus_m2.clone(),
                [
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                minus_m2.clone(),
                [
                    one.clone(),
                    two.clone(),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                context.parse("s-m2").unwrap(),
                [
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    two.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                context.parse("s-m2").unwrap(),
                [zero.clone(), zero.clone(), one, zero, two],
            ),
        ],
        vec![vec![context.parameter("s").unwrap()]],
        (0..5)
            .map(|index| context.parameter(&format!("nu{index}")).unwrap())
            .collect(),
    )
    .unwrap();

    validate_all_rows(
        &family,
        &[
            vec![0, 0, 0, 0, 0],
            vec![1, 2, 3, 4, 5],
            vec![-1, 3, 0, 2, -2],
        ],
    );
}

#[test]
fn oracle_two_loop_two_external_physical_family_and_li() {
    let mut parameters = vec![
        "d".to_owned(),
        "s00".to_owned(),
        "s01".to_owned(),
        "s11".to_owned(),
    ];
    parameters.extend((0..7).map(|index| format!("m{index}")));
    parameters.extend((0..7).map(|index| format!("nu{index}")));
    let context = CoefficientContext::new(parameters);
    let zero = context.zero();
    let one = context.one();
    let two = context.integer(2);
    let mass = |index: usize| context.parse(&format!("-m{index}")).unwrap();
    let shifted_mass =
        |invariant: &str, index: usize| context.parse(&format!("{invariant}-m{index}")).unwrap();

    // Coordinate order is
    // k0^2,k0.k1,k1^2,k0.p0,k0.p1,k1.p0,k1.p1.  The seven physical
    // propagators form a complete basis, so this checks all eight ordinary
    // rows and the genuinely multi-loop LI row without a production inverse
    // or derivative cache in the oracle.
    let family = IntegralFamily::new(
        "oracle-2l-e2-physical-li",
        vec!["k0".into(), "k1".into()],
        vec!["p0".into(), "p1".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            affine(
                mass(0),
                [
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                mass(1),
                [
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                mass(2),
                [
                    one.clone(),
                    two.clone(),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                shifted_mass("s00", 3),
                [
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    two.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                shifted_mass("s11", 4),
                [
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    two.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                shifted_mass("s00", 5),
                [
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    two.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                shifted_mass("s11", 6),
                [
                    zero.clone(),
                    zero.clone(),
                    one,
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    two,
                ],
            ),
        ],
        vec![
            vec![
                context.parameter("s00").unwrap(),
                context.parameter("s01").unwrap(),
            ],
            vec![
                context.parameter("s01").unwrap(),
                context.parameter("s11").unwrap(),
            ],
        ],
        (0..7)
            .map(|index| context.parameter(&format!("nu{index}")).unwrap())
            .collect(),
    )
    .unwrap();

    validate_all_rows(
        &family,
        &[
            vec![0, 0, 0, 0, 0, 0, 0],
            vec![1, 2, 3, 4, 5, 6, 7],
            vec![-2, 3, 0, -1, 4, 2, -3],
        ],
    );
}

#[test]
fn oracle_three_loop_tetrahedron_all_nine_ibps() {
    let context = CoefficientContext::new(["d", "m2", "nu0", "nu1", "nu2", "nu3", "nu4", "nu5"]);
    let zero = context.zero();
    let one = context.one();
    let minus_two = context.integer(-2);
    let minus_m2 = context.parse("-m2").unwrap();
    let family = IntegralFamily::new(
        "oracle-3l-tetrahedron",
        vec!["k0".into(), "k1".into(), "k2".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        // Routings k0,k1,k2,k2-k0,k0-k1,k1-k2 in coordinate order
        // k0^2,k0.k1,k0.k2,k1^2,k1.k2,k2^2.
        vec![
            affine(
                minus_m2.clone(),
                [
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                minus_m2.clone(),
                [
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                minus_m2.clone(),
                [
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                ],
            ),
            affine(
                minus_m2.clone(),
                [
                    one.clone(),
                    zero.clone(),
                    minus_two.clone(),
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                ],
            ),
            affine(
                minus_m2.clone(),
                [
                    one.clone(),
                    minus_two.clone(),
                    zero.clone(),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                minus_m2,
                [
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    one,
                    minus_two,
                    context.one(),
                ],
            ),
        ],
        Vec::new(),
        (0..6)
            .map(|index| context.parameter(&format!("nu{index}")).unwrap())
            .collect(),
    )
    .unwrap();

    validate_all_rows(
        &family,
        &[
            vec![0, 0, 0, 0, 0, 0],
            vec![1, 1, 1, 1, 1, 1],
            vec![2, -1, 3, 0, 4, -2],
        ],
    );
}

#[test]
fn oracle_five_loop_complete_massive_vacuum_all_twenty_five_ibps() {
    const LOOPS: usize = 5;
    const DENOMINATORS: usize = LOOPS * (LOOPS + 1) / 2;
    let mut parameters = vec!["d".to_owned(), "m2".to_owned()];
    parameters.extend((0..DENOMINATORS).map(|index| format!("nu{index}")));
    let context = CoefficientContext::new(parameters);
    let zero = context.zero();
    let one = context.one();
    let minus_two = context.integer(-2);
    let minus_m2 = context.parse("-m2").unwrap();
    let mut denominators = Vec::new();

    // A physical complete massive-vacuum basis: k_i^2-m2 followed by every
    // (k_i-k_j)^2-m2.  This is a test topology only; the generator receives
    // no loop-count dispatch or expected relation coefficients.
    for loop_index in 0..LOOPS {
        let mut coefficients = vec![zero.clone(); DENOMINATORS];
        coefficients[vacuum_coordinate_position(LOOPS, loop_index, loop_index)] = one.clone();
        denominators.push(affine(minus_m2.clone(), coefficients));
    }
    for left in 0..LOOPS {
        for right in left + 1..LOOPS {
            let mut coefficients = vec![zero.clone(); DENOMINATORS];
            coefficients[vacuum_coordinate_position(LOOPS, left, left)] = one.clone();
            coefficients[vacuum_coordinate_position(LOOPS, left, right)] = minus_two.clone();
            coefficients[vacuum_coordinate_position(LOOPS, right, right)] = one.clone();
            denominators.push(affine(minus_m2.clone(), coefficients));
        }
    }
    assert_eq!(denominators.len(), DENOMINATORS);

    let family = IntegralFamily::new(
        "oracle-5l-complete-massive-vacuum",
        (0..LOOPS).map(|index| format!("k{index}")).collect(),
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        (0..DENOMINATORS)
            .map(|index| context.parameter(&format!("nu{index}")).unwrap())
            .collect(),
    )
    .unwrap();

    validate_all_rows(
        &family,
        &[
            vec![1; DENOMINATORS],
            (0..DENOMINATORS)
                .map(|index| i64::try_from(index % 5).unwrap() - 2)
                .collect(),
        ],
    );
}
