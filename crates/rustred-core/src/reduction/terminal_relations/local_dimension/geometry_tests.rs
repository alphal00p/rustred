use super::*;
use crate::family::AffineDenominator;

const LIMITS: ExactAlgebraLimits = ExactAlgebraLimits::unbounded_resources();

fn key(values: &[i64]) -> IntegralKey {
    IntegralKey::try_new(values.iter().copied()).unwrap()
}

fn bubble_parent(shifts: bool) -> IntegralFamily {
    let c = CoefficientContext::new(["d", "m0", "m1", "m2", "nu"]);
    // Coordinates K²,K.b,K.c,b²,b.c,c². The negative factor can force c
    // into the spectator span even though only K,b form the positive bubble.
    let rows = [
        [1, 0, 0, 0, 0, 0],
        [1, -2, 0, 1, 0, 0],
        [1, 0, -2, 0, 0, 1],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 1],
        [0, 0, 0, 0, 1, 0],
    ];
    let denominators = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mass = c.parameter(["m0", "m1", "m2"][i % 3]).unwrap();
            AffineDenominator::new(-mass, row.iter().map(|&x| c.integer(x)).collect())
        })
        .collect();
    let mut nu = vec![c.zero(); rows.len()];
    if shifts {
        nu[2] = c.parameter("nu").unwrap();
    }
    IntegralFamily::new(
        "local-dimension-three-loop-bubble",
        vec!["K".into(), "b".into(), "c".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        denominators,
        vec![],
        nu,
    )
    .unwrap()
}

#[test]
fn native_kernel_handles_empty_full_rank_and_multiple_free_columns() {
    assert_eq!(
        kernel(&[], 3).unwrap(),
        vec![
            vec![Rational::from(1), Rational::from(0), Rational::from(0)],
            vec![Rational::from(0), Rational::from(1), Rational::from(0)],
            vec![Rational::from(0), Rational::from(0), Rational::from(1)],
        ]
    );
    assert!(
        kernel(&[vec![1.into(), 0.into()], vec![0.into(), 1.into()]], 2)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        kernel(&[vec![0.into(), (1, 2).into(), (-1, 3).into()]], 3).unwrap(),
        vec![
            vec![Rational::from(1), Rational::from(0), Rational::from(0)],
            vec![Rational::from(0), Rational::from(2), Rational::from(3)],
        ]
    );
    assert_eq!(
        normalize_direction(&[(-1, 2).into(), (3, 4).into()]).unwrap(),
        vec![Rational::from(2), Rational::from(-3)]
    );
}

#[test]
fn structural_pair_direction_is_replayed_and_preserves_positive_spectators() {
    let family = bubble_parent(false);
    let directions =
        structural_directions(&family, &key(&[1, 1, 0, 1, 1, 0]), LIMITS, None).unwrap();
    assert!(directions.contains(&vec![1.into(), 0.into(), 0.into()]));
    for direction in directions {
        let geometry =
            prepare_geometry(&family, &key(&[1, 1, 0, 1, 1, 0]), &direction, LIMITS, None).unwrap();
        assert_eq!(dot(&geometry.direction, &geometry.kappa), Rational::from(1));
        for spectator in geometry.spectators {
            assert!(dot(&geometry.direction, &spectator.loops).is_zero());
        }
    }
}

#[test]
fn negative_numerator_enlarges_bubble_spectator_rank_without_cap() {
    let family = bubble_parent(false);
    let direction = vec![1.into(), 0.into(), 0.into()];
    let scalar =
        prepare_geometry(&family, &key(&[1, 1, 0, 1, 1, 0]), &direction, LIMITS, None).unwrap();
    let numerator = prepare_geometry(
        &family,
        &key(&[1, 1, -2, 1, 1, 0]),
        &direction,
        LIMITS,
        None,
    )
    .unwrap();
    assert_eq!(scalar.spectators.len(), 1);
    assert_eq!(numerator.spectators.len(), 2);
    assert_eq!(
        numerator.alpha,
        vec![
            Rational::from(1),
            Rational::from(1),
            Rational::from(1),
            Rational::from(0),
            Rational::from(0),
            Rational::from(0)
        ]
    );
    assert_eq!(
        numerator.gradient_contractions[1],
        LinearMomentum {
            loops: vec![1.into(), (-1).into(), 0.into()],
            external: vec![],
        }
    );
    assert_eq!(
        numerator.residuals[2],
        LinearMomentum {
            loops: vec![0.into(), 0.into(), (-1).into()],
            external: vec![],
        }
    );
}

#[test]
fn analytically_active_zero_slot_participates_and_effective_zero_drops() {
    let shifted = bubble_parent(true);
    let direction = vec![1.into(), 0.into(), 0.into()];
    let shifted_geometry = prepare_geometry(
        &shifted,
        &key(&[1, 1, 0, 1, 1, 0]),
        &direction,
        LIMITS,
        None,
    )
    .unwrap();
    assert_eq!(shifted_geometry.spectators.len(), 2);

    let old = bubble_parent(false);
    let mut shifts = old.power_shifts().to_vec();
    shifts[2] = old.coefficient_context().one();
    let cancelled = IntegralFamily::new(
        "local-dimension-effective-exponent-cancels",
        old.loop_momenta().to_vec(),
        vec![],
        old.coefficient_context().clone(),
        old.dimension().clone(),
        old.denominators().to_vec(),
        vec![],
        shifts,
    )
    .unwrap();
    let geometry = prepare_geometry(
        &cancelled,
        &key(&[1, 1, -1, 1, 1, 0]),
        &direction,
        LIMITS,
        None,
    )
    .unwrap();
    assert_eq!(geometry.spectators.len(), 1);
}

#[test]
fn scalar_product_lowering_preserves_mass_terms_and_off_diagonals() {
    let family = bubble_parent(false);
    let c = family.coefficient_context();
    let k = LinearMomentum {
        loops: vec![1.into(), 0.into(), 0.into()],
        external: vec![],
    };
    let b = LinearMomentum {
        loops: vec![0.into(), 1.into(), 0.into()],
        external: vec![],
    };
    let (mass, row) = scalar_product_coefficients(&family, &k, &k, LIMITS).unwrap();
    assert_eq!(mass, c.parameter("m0").unwrap());
    assert_eq!(
        row,
        vec![c.one(), c.zero(), c.zero(), c.zero(), c.zero(), c.zero()]
    );
    let (constant, row) = scalar_product_coefficients(&family, &k, &b, LIMITS).unwrap();
    let half = c.coefficient_fixture("1/2");
    assert_eq!(
        row,
        vec![
            half.clone(),
            -half.clone(),
            c.zero(),
            half.clone(),
            c.zero(),
            c.zero()
        ]
    );
    assert_eq!(
        constant,
        c.try_mul(
            &c.try_add(
                &c.try_sub(
                    &c.parameter("m0").unwrap(),
                    &c.parameter("m1").unwrap(),
                    LIMITS
                )
                .unwrap(),
                &c.parameter("m0").unwrap(),
                LIMITS,
            )
            .unwrap(),
            &half,
            LIMITS,
        )
        .unwrap()
    );
}

#[test]
fn external_parameter_gram_is_supported_but_parameter_momentum_geometry_is_not() {
    let c = CoefficientContext::new(["d", "m", "s", "t", "r", "z"]);
    let make = |parameter_geometry: bool| {
        let rows = vec![
            vec![
                if parameter_geometry {
                    c.parameter("z").unwrap()
                } else {
                    c.one()
                },
                c.zero(),
                c.zero(),
            ],
            vec![c.one(), c.integer(-2), c.zero()],
            vec![c.one(), c.zero(), c.integer(-2)],
        ];
        let constants = vec![
            -c.parameter("m").unwrap(),
            c.parameter("s").unwrap() - c.parameter("m").unwrap(),
            c.parameter("t").unwrap() - c.parameter("m").unwrap(),
        ];
        IntegralFamily::new(
            "local-dimension-external",
            vec!["K".into()],
            vec!["p".into(), "q".into()],
            c.clone(),
            c.parameter("d").unwrap(),
            rows.into_iter()
                .zip(constants)
                .map(|(row, constant)| AffineDenominator::new(constant, row))
                .collect(),
            vec![
                vec![c.parameter("s").unwrap(), c.parameter("r").unwrap()],
                vec![c.parameter("r").unwrap(), c.parameter("t").unwrap()],
            ],
            vec![c.zero(); 3],
        )
        .unwrap()
    };
    let family = make(false);
    let geometry = prepare_geometry(&family, &key(&[1, 1, -1]), &[1.into()], LIMITS, None).unwrap();
    assert_eq!(geometry.spectators.len(), 2);
    let (constant, coefficients) = scalar_product_coefficients(
        &family,
        &geometry.spectators[0],
        &geometry.spectators[1],
        LIMITS,
    )
    .unwrap();
    assert_eq!(constant, c.parameter("r").unwrap());
    assert!(coefficients.iter().all(|coefficient| coefficient.is_zero()));
    assert!(matches!(
        prepare_geometry(&make(true), &key(&[1, 1, 0]), &[1.into()], LIMITS, None),
        Err(GeometryError::ParameterValuedGeometry {
            denominator: 0,
            coordinate: 0
        })
    ));
}

#[test]
fn geometry_rejects_zero_wrong_arity_and_cancellation_without_mutation() {
    let family = bubble_parent(false);
    let source = key(&[1, 1, -1, 1, 1, 0]);
    assert!(matches!(
        prepare_geometry(
            &family,
            &source,
            &[0.into(), 0.into(), 0.into()],
            LIMITS,
            None
        ),
        Err(GeometryError::InvalidInput(_))
    ));
    assert!(matches!(
        prepare_geometry(&family, &source, &[1.into()], LIMITS, None),
        Err(GeometryError::InvalidInput(_))
    ));
    assert!(matches!(
        prepare_geometry(
            &family,
            &source,
            &[1.into(), 0.into(), 0.into()],
            LIMITS,
            Some(&|| true)
        ),
        Err(GeometryError::Cancelled)
    ));
    assert_eq!(source.powers(), &[1, 1, -1, 1, 1, 0]);
}

#[test]
fn noncoordinate_direction_normalizes_and_reconstructs_inactive_residuals() {
    let c = CoefficientContext::new(["d", "m"]);
    let rows = [[1, 0, 0], [1, -2, 1], [0, 0, 1]];
    let family = IntegralFamily::new(
        "local-dimension-noncoordinate",
        vec!["k1".into(), "k2".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        rows.iter()
            .map(|row| {
                AffineDenominator::new(
                    -c.parameter("m").unwrap(),
                    row.iter().map(|&x| c.integer(x)).collect(),
                )
            })
            .collect(),
        vec![],
        vec![c.zero(); rows.len()],
    )
    .unwrap();
    let geometry = prepare_geometry(
        &family,
        &key(&[1, 1, 0]),
        &[(-2).into(), (-2).into()],
        LIMITS,
        None,
    )
    .unwrap();
    assert_eq!(
        geometry.direction,
        vec![Rational::from(1), Rational::from(1)]
    );
    assert_eq!(geometry.kappa, vec![Rational::from(1), Rational::from(0)]);
    assert!(geometry.spectators.is_empty());
    assert_eq!(
        geometry.alpha,
        vec![Rational::from(1), Rational::from(0), Rational::from(1)]
    );
    assert_eq!(
        geometry.residuals[2].loops,
        vec![Rational::from(-1), Rational::from(1)]
    );
    assert_eq!(
        geometry.gradient_contractions[1].loops,
        vec![Rational::from(0), Rational::from(0)]
    );
    let numerator = prepare_geometry(
        &family,
        &key(&[1, 1, -1]),
        &[1.into(), 1.into()],
        LIMITS,
        None,
    )
    .unwrap();
    assert_eq!(numerator.spectators.len(), 1);
    assert_eq!(
        numerator.spectators[0].loops,
        vec![Rational::from(-1), Rational::from(1)]
    );
}
