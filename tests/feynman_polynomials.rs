use rustred::{
    AffineDenominator, Coefficient, CoefficientContext, FeynmanPolynomialError,
    FeynmanPolynomialLimits, IntegralFamily, SectorMask, SymanzikPolynomials,
};

fn affine(
    constant: Coefficient,
    coefficients: impl IntoIterator<Item = Coefficient>,
) -> AffineDenominator {
    AffineDenominator::new(constant, coefficients.into_iter().collect())
}

fn tadpole_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![affine(
            coefficients.parameter("m2").unwrap(),
            [coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn off_shell_bubble_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "s"]);
    let s = coefficients.parameter("s").unwrap();
    IntegralFamily::new(
        name,
        vec!["k".into()],
        vec!["p".into()],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                coefficients.zero(),
                [coefficients.one(), coefficients.zero()],
            ),
            affine(s.clone(), [coefficients.one(), coefficients.integer(2)]),
        ],
        vec![vec![s]],
        vec![coefficients.zero(), coefficients.zero()],
    )
    .unwrap()
}

fn sunset_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let m2 = coefficients.parameter("m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                m2.clone(),
                [coefficients.one(), coefficients.zero(), coefficients.zero()],
            ),
            affine(
                m2.clone(),
                [coefficients.zero(), coefficients.zero(), coefficients.one()],
            ),
            affine(
                m2,
                [
                    coefficients.one(),
                    coefficients.integer(-2),
                    coefficients.one(),
                ],
            ),
        ],
        Vec::new(),
        vec![coefficients.zero(); 3],
    )
    .unwrap()
}

fn two_loop_one_external_family(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "s"]);
    let s = coefficients.parameter("s").unwrap();
    let denominators = (0..5)
        .map(|coordinate| {
            affine(
                coefficients.zero(),
                (0..5).map(|candidate| {
                    if candidate == coordinate {
                        coefficients.one()
                    } else {
                        coefficients.zero()
                    }
                }),
            )
        })
        .collect();
    IntegralFamily::new(
        name,
        vec!["k0".into(), "k1".into()],
        vec!["p".into()],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        denominators,
        vec![vec![s]],
        vec![coefficients.zero(); 5],
    )
    .unwrap()
}

#[test]
fn one_loop_tadpole_has_expected_u_f_g() {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let m2 = coefficients.parameter("m2").unwrap();
    let family = IntegralFamily::new(
        "symanzik-tadpole",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![affine(m2.clone(), [coefficients.one()])],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap();

    let symanzik = SymanzikPolynomials::try_from_family(&family).unwrap();
    assert_eq!(symanzik.u().term_count(), 1);
    assert_eq!(symanzik.u().coefficient(&[1]), Some(&coefficients.one()));
    assert_eq!(symanzik.f().term_count(), 1);
    assert_eq!(symanzik.f().coefficient(&[2]), Some(&m2));
    assert_eq!(symanzik.g().term_count(), 2);
    assert_eq!(symanzik.g().coefficient(&[1]), Some(&coefficients.one()));
    assert_eq!(symanzik.g().coefficient(&[2]), Some(&m2));

    let gradient = symanzik.try_gradient().unwrap();
    assert_eq!(gradient.len(), 1);
    assert_eq!(gradient[0].coefficient(&[0]), Some(&coefficients.one()));
    assert_eq!(
        gradient[0].coefficient(&[1]),
        Some(
            &coefficients
                .try_mul(&coefficients.integer(2), &m2, Default::default())
                .unwrap()
        )
    );
    let empty = SectorMask::try_new([false]).unwrap();
    assert!(
        symanzik
            .try_restrict_face(symanzik.g(), &empty)
            .unwrap()
            .is_zero()
    );
}

#[test]
fn off_shell_bubble_cancels_the_x1_squared_term() {
    let coefficients = CoefficientContext::new(["d", "s"]);
    let s = coefficients.parameter("s").unwrap();
    let family = IntegralFamily::new(
        "symanzik-bubble",
        vec!["k".into()],
        vec!["p".into()],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                coefficients.zero(),
                [coefficients.one(), coefficients.zero()],
            ),
            affine(s.clone(), [coefficients.one(), coefficients.integer(2)]),
        ],
        vec![vec![s.clone()]],
        vec![coefficients.zero(), coefficients.zero()],
    )
    .unwrap();

    let symanzik = SymanzikPolynomials::try_from_family(&family).unwrap();
    assert_eq!(symanzik.u().term_count(), 2);
    assert_eq!(symanzik.u().coefficient(&[1, 0]), Some(&coefficients.one()));
    assert_eq!(symanzik.u().coefficient(&[0, 1]), Some(&coefficients.one()));
    assert_eq!(symanzik.f().term_count(), 1);
    assert_eq!(symanzik.f().coefficient(&[1, 1]), Some(&s));
    assert_eq!(symanzik.f().coefficient(&[0, 2]), None);

    let first_pinched = SectorMask::try_new([true, false]).unwrap();
    let face = symanzik
        .try_restrict_face(symanzik.g(), &first_pinched)
        .unwrap();
    assert_eq!(face.coefficient(&[1, 0]), Some(&coefficients.one()));
    assert_eq!(face.term_count(), 1);
}

#[test]
fn two_loop_sunset_has_expected_symanzik_polynomials() {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let m2 = coefficients.parameter("m2").unwrap();
    let family = IntegralFamily::new(
        "symanzik-sunset",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                m2.clone(),
                [coefficients.one(), coefficients.zero(), coefficients.zero()],
            ),
            affine(
                m2.clone(),
                [coefficients.zero(), coefficients.zero(), coefficients.one()],
            ),
            affine(
                m2.clone(),
                [
                    coefficients.one(),
                    coefficients.integer(-2),
                    coefficients.one(),
                ],
            ),
        ],
        Vec::new(),
        vec![coefficients.zero(); 3],
    )
    .unwrap();

    let symanzik = SymanzikPolynomials::try_from_family(&family).unwrap();
    for exponent in [[1, 1, 0], [1, 0, 1], [0, 1, 1]] {
        assert_eq!(
            symanzik.u().coefficient(&exponent),
            Some(&coefficients.one())
        );
    }
    assert_eq!(symanzik.u().term_count(), 3);
    for exponent in [
        [2, 1, 0],
        [2, 0, 1],
        [1, 2, 0],
        [0, 2, 1],
        [1, 0, 2],
        [0, 1, 2],
    ] {
        assert_eq!(symanzik.f().coefficient(&exponent), Some(&m2));
    }
    assert_eq!(
        symanzik.f().coefficient(&[1, 1, 1]),
        Some(
            &coefficients
                .try_mul(&coefficients.integer(3), &m2, Default::default())
                .unwrap()
        )
    );
    assert_eq!(symanzik.f().term_count(), 7);
}

#[test]
fn aggregate_term_budget_includes_assembly_determinant_and_gram_work() {
    let family = off_shell_bubble_family("symanzik-aggregate-work");
    let below = FeynmanPolynomialLimits {
        max_term_operations: 33,
        ..FeynmanPolynomialLimits::default()
    };
    assert!(matches!(
        SymanzikPolynomials::try_from_family_with_limits(&family, below),
        Err(FeynmanPolynomialError::ResourceLimit {
            resource: "aggregate Feynman polynomial operations",
            requested: 34,
            limit: 33,
        })
    ));

    let exact = FeynmanPolynomialLimits {
        max_term_operations: 34,
        ..FeynmanPolynomialLimits::default()
    };
    SymanzikPolynomials::try_from_family_with_limits(&family, exact).unwrap();
}

#[test]
fn determinant_budget_is_shared_by_u_and_every_adjugate_minor() {
    let family = two_loop_one_external_family("symanzik-aggregate-determinants");
    let below = FeynmanPolynomialLimits {
        max_determinant_operations: 7,
        ..FeynmanPolynomialLimits::default()
    };
    assert!(matches!(
        SymanzikPolynomials::try_from_family_with_limits(&family, below),
        Err(FeynmanPolynomialError::ResourceLimit {
            resource: "aggregate determinant operations",
            requested: 8,
            limit: 7,
        })
    ));

    let exact = FeynmanPolynomialLimits {
        max_determinant_operations: 8,
        ..FeynmanPolynomialLimits::default()
    };
    SymanzikPolynomials::try_from_family_with_limits(&family, exact).unwrap();
}

#[test]
fn vacuum_skips_adjugate_limit_without_changing_deterministic_results() {
    let family = sunset_family("symanzik-vacuum-skips-adjugate");
    let unrestricted = SymanzikPolynomials::try_from_family(&family).unwrap();
    let limits = FeynmanPolynomialLimits {
        max_adjugate_minors: 1,
        ..FeynmanPolynomialLimits::default()
    };

    let first = SymanzikPolynomials::try_from_family_with_limits(&family, limits).unwrap();
    let second = SymanzikPolynomials::try_from_family_with_limits(&family, limits).unwrap();

    assert_eq!(unrestricted.u().stable_string(), first.u().stable_string());
    assert_eq!(unrestricted.f().stable_string(), first.f().stable_string());
    assert_eq!(unrestricted.g().stable_string(), first.g().stable_string());
    assert_eq!(first.u().stable_string(), second.u().stable_string());
    assert_eq!(first.f().stable_string(), second.f().stable_string());
    assert_eq!(first.g().stable_string(), second.g().stable_string());
    assert_eq!(first.family_domain(), family.domain());
    assert_eq!(first.u().term_count(), 3);
    assert_eq!(first.f().term_count(), 7);
}

#[test]
fn non_vacuum_still_enforces_adjugate_minor_limit() {
    let family = two_loop_one_external_family("symanzik-non-vacuum-needs-adjugate");
    let limits = FeynmanPolynomialLimits {
        max_adjugate_minors: 1,
        ..FeynmanPolynomialLimits::default()
    };

    assert!(matches!(
        SymanzikPolynomials::try_from_family_with_limits(&family, limits),
        Err(FeynmanPolynomialError::ResourceLimit {
            resource: "adjugate minors",
            requested: 4,
            limit: 1,
        })
    ));
}

#[test]
fn dense_exponent_entry_limit_is_checked_at_the_exact_boundary() {
    let family = tadpole_family("symanzik-exponent-entry-limit");
    let below = FeynmanPolynomialLimits {
        max_exponent_entries: 1,
        ..FeynmanPolynomialLimits::default()
    };
    assert!(matches!(
        SymanzikPolynomials::try_from_family_with_limits(&family, below),
        Err(FeynmanPolynomialError::ResourceLimit {
            requested: 2,
            limit: 1,
            ..
        })
    ));

    let exact = FeynmanPolynomialLimits {
        max_exponent_entries: 2,
        ..FeynmanPolynomialLimits::default()
    };
    SymanzikPolynomials::try_from_family_with_limits(&family, exact).unwrap();
}

#[test]
fn symanzik_result_retains_family_domain_after_visible_cancellation() {
    let coefficients = CoefficientContext::new(["d", "h", "m"]);
    let family = IntegralFamily::new(
        "symanzik-domain-retention",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![affine(
            coefficients.parse("h*m").unwrap(),
            [coefficients.parse("1/h").unwrap()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap();
    let symanzik = SymanzikPolynomials::try_from_family(&family).unwrap();

    assert_eq!(
        symanzik.f().coefficient(&[2]),
        Some(&coefficients.parameter("m").unwrap())
    );
    assert_eq!(symanzik.family_domain(), family.domain());
    assert_eq!(symanzik.context().family_domain(), family.domain());
    assert!(symanzik.family_domain().conditions().count() >= 2);
}

#[test]
fn feynman_parameter_cannot_alias_a_base_field_symbol() {
    let coefficients = CoefficientContext::new(["d", "feynman_x_0"]);
    let family = IntegralFamily::new(
        "symanzik-symbol-role-collision",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![affine(coefficients.zero(), [coefficients.one()])],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap();

    assert!(matches!(
        SymanzikPolynomials::try_from_family(&family),
        Err(FeynmanPolynomialError::FeynmanBaseSymbolCollision {
            parameter: 0,
            ref base_parameter,
        }) if base_parameter == "feynman_x_0"
    ));
}

#[test]
fn foreign_family_polynomial_is_rejected_before_face_work() {
    let left = SymanzikPolynomials::try_from_family(&tadpole_family("symanzik-left")).unwrap();
    let right = SymanzikPolynomials::try_from_family(&tadpole_family("symanzik-right")).unwrap();
    let top = SectorMask::try_new([true]).unwrap();
    assert!(matches!(
        left.try_restrict_face(right.g(), &top),
        Err(FeynmanPolynomialError::ForeignPolynomialContext)
    ));
}

#[test]
fn two_loop_two_external_singular_gram_matches_direct_completion() {
    let coefficients = CoefficientContext::new(["d", "g"]);
    let g = coefficients.parameter("g").unwrap();
    let denominators = (0..7)
        .map(|coordinate| {
            affine(
                coefficients.zero(),
                (0..7).map(|candidate| {
                    if candidate == coordinate {
                        coefficients.one()
                    } else {
                        coefficients.zero()
                    }
                }),
            )
        })
        .collect::<Vec<_>>();
    let family = IntegralFamily::new(
        "symanzik-two-loop-two-external-singular-gram",
        vec!["k0".into(), "k1".into()],
        vec!["p0".into(), "p1".into()],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        denominators,
        vec![vec![g.clone(), g.clone()], vec![g.clone(), g.clone()]],
        vec![coefficients.zero(); 7],
    )
    .unwrap();
    let symanzik = SymanzikPolynomials::try_from_family(&family).unwrap();

    // A=[[x0,x1/2],[x1/2,x2]], Q=[[x3/2,x4/2],[x5/2,x6/2]],
    // H=g[[1,1],[1,1]].  Thus F=-g[x2*s0^2-x1*s0*s1+x0*s1^2]
    // with s0=(x3+x4)/2 and s1=(x5+x6)/2.
    assert_eq!(
        symanzik.u().coefficient(&[1, 0, 1, 0, 0, 0, 0]),
        Some(&coefficients.one())
    );
    assert_eq!(
        symanzik.u().coefficient(&[0, 2, 0, 0, 0, 0, 0]),
        Some(&coefficients.rational(rustred::ExactRational::new(-1, 4)))
    );
    assert_eq!(
        symanzik.f().coefficient(&[0, 0, 1, 2, 0, 0, 0]),
        Some(&coefficients.parse("-g/4").unwrap())
    );
    assert_eq!(
        symanzik.f().coefficient(&[0, 0, 1, 1, 1, 0, 0]),
        Some(&coefficients.parse("-g/2").unwrap())
    );
    assert_eq!(
        symanzik.f().coefficient(&[0, 1, 0, 1, 0, 1, 0]),
        Some(&coefficients.parse("g/4").unwrap())
    );
    assert_eq!(
        symanzik.f().coefficient(&[1, 0, 0, 0, 0, 2, 0]),
        Some(&coefficients.parse("-g/4").unwrap())
    );
}
