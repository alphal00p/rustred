//! Public-API regressions for non-vacuum, unequal-mass two-loop families.
//!
//! Mass symbols denote independent squared masses in the Euclidean convention
//! `D = q^2 + m_i`. Auxiliary denominators complete the scalar-product basis;
//! they are not extra physical lines of the indicated topology.

use std::collections::BTreeMap;

use rustred::algebra::{Coefficient, CoefficientContext, IndexedCoefficient};
use rustred::family::{AffineDenominator, ContractionMomentum, IntegralFamily};
use rustred::identity::{
    IntegralShift, ParametricIbpGenerator, ParametricRelation, TranslatedSourceLimits,
};

struct Fixture {
    family: IntegralFamily,
    /// Independently specified momentum routings in [k1, k2, p1, ...] order.
    routings: Vec<Vec<i64>>,
}

fn parameter(base: &CoefficientContext, name: &str) -> Coefficient {
    base.parameter(name).unwrap()
}

fn fixture(four_point: bool) -> Fixture {
    let externals = if four_point { 3 } else { 2 };
    let arity = 3 + 2 * externals;
    let mut parameters = vec!["d".into(), "s1".into(), "s2".into(), "t12".into()];
    if four_point {
        parameters.extend(["s3".into(), "t13".into(), "t23".into()]);
    }
    parameters.extend((1..=arity).map(|i| format!("m{i}")));
    let base = CoefficientContext::try_new(parameters).unwrap();
    let p = |name| parameter(&base, name);
    let s12 = &(&p("s1") + &p("s2")) + &(&base.integer(2) * &p("t12"));
    let (constants, rows, routings, gram) = if four_point {
        // Off-shell planar double box, seven physical lines and two auxiliary
        // denominators (D8, D9). The six independent Gram entries retain all
        // external virtualities and both independent scattering invariants.
        let s123 = &(&(&s12 + &p("s3")) + &(&base.integer(2) * &p("t13")))
            + &(&base.integer(2) * &p("t23"));
        (
            vec![
                p("m1"),
                &p("s1") + &p("m2"),
                &s12 + &p("m3"),
                p("m4"),
                p("m5"),
                &s12 + &p("m6"),
                &s123 + &p("m7"),
                &p("s3") + &p("m8"),
                &p("s1") + &p("m9"),
            ],
            vec![
                vec![1, 0, 0, 0, 0, 0, 0, 0, 0],
                vec![1, 0, 0, 2, 0, 0, 0, 0, 0],
                vec![1, 0, 0, 2, 2, 0, 0, 0, 0],
                vec![1, -2, 1, 0, 0, 0, 0, 0, 0],
                vec![0, 0, 1, 0, 0, 0, 0, 0, 0],
                vec![0, 0, 1, 0, 0, 0, 2, 2, 0],
                vec![0, 0, 1, 0, 0, 0, 2, 2, 2],
                vec![1, 0, 0, 0, 0, 2, 0, 0, 0],
                vec![0, 0, 1, 0, 0, 0, 2, 0, 0],
            ],
            vec![
                vec![1, 0, 0, 0, 0],
                vec![1, 0, 1, 0, 0],
                vec![1, 0, 1, 1, 0],
                vec![1, -1, 0, 0, 0],
                vec![0, 1, 0, 0, 0],
                vec![0, 1, 1, 1, 0],
                vec![0, 1, 1, 1, 1],
                vec![1, 0, 0, 0, 1],
                vec![0, 1, 1, 0, 0],
            ],
            vec![
                vec![p("s1"), p("t12"), p("t13")],
                vec![p("t12"), p("s2"), p("t23")],
                vec![p("t13"), p("t23"), p("s3")],
            ],
        )
    } else {
        // Off-shell three-point ladder: six physical lines, with D7 an
        // auxiliary denominator. p3 = -(p1+p2), so p3^2 = s1+s2+2*t12.
        (
            vec![
                p("m1"),
                &p("s1") + &p("m2"),
                &s12 + &p("m3"),
                p("m4"),
                &s12 + &p("m5"),
                p("m6"),
                &p("s1") + &p("m7"),
            ],
            vec![
                vec![1, 0, 0, 0, 0, 0, 0],
                vec![1, 0, 0, 2, 0, 0, 0],
                vec![1, 0, 0, 2, 2, 0, 0],
                vec![0, 0, 1, 0, 0, 0, 0],
                vec![0, 0, 1, 0, 0, 2, 2],
                vec![1, -2, 1, 0, 0, 0, 0],
                vec![0, 0, 1, 0, 0, 2, 0],
            ],
            vec![
                vec![1, 0, 0, 0],
                vec![1, 0, 1, 0],
                vec![1, 0, 1, 1],
                vec![0, 1, 0, 0],
                vec![0, 1, 1, 1],
                vec![1, -1, 0, 0],
                vec![0, 1, 1, 0],
            ],
            vec![vec![p("s1"), p("t12")], vec![p("t12"), p("s2")]],
        )
    };
    let denominators = constants
        .into_iter()
        .zip(rows)
        .map(|(constant, row)| {
            AffineDenominator::new(constant, row.into_iter().map(|x| base.integer(x)).collect())
        })
        .collect();
    Fixture {
        family: IntegralFamily::new(
            if four_point {
                "unequal-mass-double-box"
            } else {
                "unequal-mass-vertex"
            },
            vec!["k1".into(), "k2".into()],
            (1..=externals).map(|i| format!("p{i}")).collect(),
            base.clone(),
            p("d"),
            denominators,
            gram,
            vec![base.zero(); arity],
        )
        .unwrap(),
        routings,
    }
}

fn assert_equal(actual: &Coefficient, expected: &Coefficient) {
    assert!(
        (actual - expected).is_zero(),
        "actual={actual}, expected={expected}"
    );
}

fn check_contractions(fixture: &Fixture) {
    let family = &fixture.family;
    let base = family.coefficient_context();
    let e = family.external_count();
    let k = family.denominator_count();
    assert_eq!(family.loop_count(), 2);
    assert_eq!(k, 3 + 2 * e);
    // A constant determinant guard is retained, but no kinematic or mass
    // specialization is needed to invert these denominator bases.
    assert!(family.domain().conditions().all(|condition| {
        condition.polynomial().is_constant() && !condition.polynomial().is_zero()
    }));
    for (denominator, q) in fixture.routings.iter().enumerate() {
        for differentiated_loop in 0..2 {
            for v in 0..(2 + e) {
                let contraction = if v < 2 {
                    ContractionMomentum::Loop(v)
                } else {
                    ContractionMomentum::External(v - 2)
                };
                let image = family
                    .derivative_contraction(denominator, differentiated_loop, contraction)
                    .unwrap();
                // Independently differentiate the routed momentum square:
                // v . d(q^2 + m_i)/dk_l = 2*q_l*(v.q). This oracle never
                // reads the cached derivative or inverse denominator basis.
                let factor = 2 * q[differentiated_loop];
                let mut expected = vec![base.zero(); k + 1];
                if v < 2 {
                    for (other_loop, &routing) in q[..2].iter().enumerate() {
                        expected[1 + v + other_loop] = base.integer(factor * routing);
                    }
                    for external in 0..e {
                        expected[1 + 3 + v * e + external] = base.integer(factor * q[2 + external]);
                    }
                } else {
                    for loop_index in 0..2 {
                        expected[1 + 3 + loop_index * e + v - 2] =
                            base.integer(factor * q[loop_index]);
                    }
                    for external in 0..e {
                        expected[0] = &expected[0]
                            + &(&base.integer(factor * q[2 + external])
                                * &family.external_gram()[v - 2][external]);
                    }
                }
                // Expand the returned affine form back in scalar products.
                let mut actual = vec![base.zero(); k + 1];
                actual[0] = image.constant().clone();
                for (weight, d) in image
                    .denominator_coefficients()
                    .iter()
                    .zip(family.denominators())
                {
                    actual[0] = &actual[0] + &(weight * d.constant());
                    for (cell, coefficient) in actual[1..].iter_mut().zip(d.coefficients()) {
                        *cell = &*cell + &(weight * coefficient);
                    }
                }
                for (actual, expected) in actual.iter().zip(&expected) {
                    assert_equal(actual, expected);
                }
            }
        }
    }
}

#[test]
fn unequal_mass_vertex_retains_all_three_external_scales_in_exact_contractions() {
    check_contractions(&fixture(false));
}

#[test]
fn unequal_mass_double_box_retains_six_external_invariants_in_exact_contractions() {
    check_contractions(&fixture(true));
}

// Each tuple means coefficient n_power * expression at shift +e_raise-e_lower.
// Indices here are one-based, matching the hand-written equations below.
type ExpectedTerm<'a> = (Option<usize>, Option<usize>, Option<usize>, &'a str);

fn assert_row(
    generator: &ParametricIbpGenerator<'_>,
    row: &ParametricRelation,
    expected: &[ExpectedTerm<'_>],
) {
    let context = generator.context();
    let mut terms = BTreeMap::<Vec<i64>, IndexedCoefficient>::new();
    for &(raise, lower, power, expression) in expected {
        let mut shift = vec![0; context.index_count()];
        if let Some(i) = raise {
            shift[i - 1] += 1;
        }
        if let Some(i) = lower {
            shift[i - 1] -= 1;
        }
        let mut value = context
            .parse_expression_with_limits(expression, Default::default())
            .unwrap();
        if let Some(i) = power {
            value = context
                .mul_with_limits(&context.index(i - 1).unwrap(), &value, Default::default())
                .unwrap();
        }
        let cell = terms.entry(shift).or_insert_with(|| context.zero());
        *cell = context
            .add_with_limits(cell, &value, Default::default())
            .unwrap();
    }
    terms.retain(|_, value| !value.is_zero());
    let actual = row
        .terms()
        .iter()
        .map(|(shift, value)| (shift.values().to_vec(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, terms);
}

#[test]
fn vertex_ibps_have_exact_distinct_mass_and_external_scale_coefficients() {
    let fixture = fixture(false);
    let generator = ParametricIbpGenerator::try_new(&fixture.family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    assert_eq!(batch.len(), 8); // L*(L+E), not the vacuum-only L^2 span.
    let generated = (0..batch.len()).map(|i| batch.generate(i)).collect();
    let rows = batch.complete(generated).unwrap().into_relations();
    // d/dk1 . k1, obtained by differentiating the six physical propagators.
    assert_row(
        &generator,
        &rows[0],
        &[
            (None, None, None, "d"),
            (None, None, Some(1), "-2"),
            (None, None, Some(2), "-1"),
            (None, None, Some(3), "-1"),
            (None, None, Some(6), "-1"),
            (Some(1), None, Some(1), "2*m1"),
            (Some(2), None, Some(2), "s1+m1+m2"),
            (Some(2), Some(1), Some(2), "-1"),
            (Some(3), None, Some(3), "s1+s2+2*t12+m1+m3"),
            (Some(3), Some(1), Some(3), "-1"),
            (Some(6), None, Some(6), "m1-m4+m6"),
            (Some(6), Some(1), Some(6), "-1"),
            (Some(6), Some(4), Some(6), "1"),
        ],
    );
    // d/dk1 . p2. In particular the bridge propagator retains m3-m2-m5+m7.
    assert_row(
        &generator,
        &rows[6],
        &[
            (None, None, Some(2), "1"),
            (None, None, Some(3), "-1"),
            (Some(1), None, Some(1), "s2+2*t12+m3-m2"),
            (Some(1), Some(3), Some(1), "-1"),
            (Some(1), Some(2), Some(1), "1"),
            (Some(2), None, Some(2), "s2+m3-m2"),
            (Some(2), Some(3), Some(2), "-1"),
            (Some(3), None, Some(3), "-s2+m3-m2"),
            (Some(3), Some(2), Some(3), "1"),
            (Some(6), None, Some(6), "m3-m2-m5+m7"),
            (Some(6), Some(3), Some(6), "-1"),
            (Some(6), Some(2), Some(6), "1"),
            (Some(6), Some(5), Some(6), "1"),
            (Some(6), Some(7), Some(6), "-1"),
        ],
    );
}

#[test]
fn double_box_external_ibp_retains_off_diagonal_gram_entries_and_mass_differences() {
    let fixture = fixture(true);
    let generator = ParametricIbpGenerator::try_new(&fixture.family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    assert_eq!(batch.len(), 10);
    let generated = (0..batch.len()).map(|i| batch.generate(i)).collect();
    let rows = batch.complete(generated).unwrap().into_relations();
    // d/dk2 . p3, including the negative k2 routing of D4=(k1-k2)^2+m4.
    assert_row(
        &generator,
        &rows[9],
        &[
            (None, None, Some(6), "1"),
            (None, None, Some(7), "-1"),
            (Some(4), None, Some(4), "m1-m8+2*t13+2*t23+m7-m6"),
            (Some(4), Some(1), Some(4), "-1"),
            (Some(4), Some(8), Some(4), "1"),
            (Some(4), Some(7), Some(4), "-1"),
            (Some(4), Some(6), Some(4), "1"),
            (Some(5), None, Some(5), "s3+2*t13+2*t23+m7-m6"),
            (Some(5), Some(7), Some(5), "-1"),
            (Some(5), Some(6), Some(5), "1"),
            (Some(6), None, Some(6), "s3+m7-m6"),
            (Some(6), Some(7), Some(6), "-1"),
            (Some(7), None, Some(7), "-s3+m7-m6"),
            (Some(7), Some(6), Some(7), "1"),
            (Some(9), None, Some(9), "s3+2*t23+m7-m6"),
            (Some(9), Some(7), Some(9), "-1"),
            (Some(9), Some(6), Some(9), "1"),
        ],
    );
}

#[test]
fn translated_multiscale_sources_replay_at_dotted_pinched_and_numerator_indices() {
    for four_point in [false, true] {
        let fixture = fixture(four_point);
        let generator = ParametricIbpGenerator::try_new(&fixture.family).unwrap();
        let batch = generator.prepare_ordinary_ibp().unwrap();
        let generated = (0..batch.len()).map(|i| batch.generate(i)).collect();
        let completed = batch.complete(generated).unwrap();
        let k = fixture.family.denominator_count();
        let mut offset = vec![0; k];
        offset[0] = 2;
        offset[1] = -1;
        offset[k - 1] = -2;
        let translated = generator
            .translate_completed_source_rows(
                &completed,
                [IntegralShift::try_new(offset.clone()).unwrap()],
                TranslatedSourceLimits::default(),
            )
            .unwrap();
        let original = completed.into_relations();
        assert_eq!(translated.len(), original.len());
        assert!(translated.is_complete_ordinary());
        for (source, shifted) in original.iter().zip(translated.sources()) {
            assert_eq!(source.row_id(), shifted.row_id());
            assert_eq!(source.terms().len(), shifted.terms().len());
            for (shift, coefficient) in source.terms() {
                let expected_shift = shift
                    .values()
                    .iter()
                    .zip(&offset)
                    .map(|(a, b)| a + b)
                    .collect::<Vec<_>>();
                let actual = shifted
                    .terms()
                    .iter()
                    .find(|(s, _)| s.values() == expected_shift)
                    .map(|(_, c)| c)
                    .unwrap();
                for first_power in [-2, 0, 1, 3] {
                    let mut indices = vec![1; k];
                    indices[0] = first_power;
                    indices[1] = 2;
                    indices[k - 1] = -1;
                    let shifted_indices = indices
                        .iter()
                        .zip(&offset)
                        .map(|(a, b)| a + b)
                        .collect::<Vec<_>>();
                    let expected = generator
                        .context()
                        .specialize(coefficient, &shifted_indices, Default::default())
                        .unwrap()
                        .0;
                    let actual = generator
                        .context()
                        .specialize(actual, &indices, Default::default())
                        .unwrap()
                        .0;
                    assert_equal(&actual, &expected);
                }
            }
        }
    }
}
