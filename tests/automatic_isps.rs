use rustred::{
    AffineDenominator, AutomaticIspCompletion, AutomaticIspCompletionError,
    AutomaticIspCompletionLimits, CoefficientContext, ExactAlgebraError, GuardOrigin,
    ParametricIbpGenerator, ScalarProductCoordinate,
};

fn affine(
    constant: rustred::Coefficient,
    coefficients: impl IntoIterator<Item = rustred::Coefficient>,
) -> AffineDenominator {
    AffineDenominator::new(constant, coefficients.into_iter().collect())
}

#[test]
fn rustred_order_completes_two_loop_propagators_with_loop_external_isps() {
    let context = CoefficientContext::new(["d", "m0", "m1", "m2", "s", "nu0", "nu1", "nu2"]);
    let zero = context.zero();
    let one = context.one();
    let completion = AutomaticIspCompletion::try_new(
        "automatic-isp-2l-e1",
        vec!["k0".into(), "k1".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            affine(
                context.parse("-m0").unwrap(),
                [
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                context.parse("-m1").unwrap(),
                [
                    zero.clone(),
                    zero.clone(),
                    one.clone(),
                    zero.clone(),
                    zero.clone(),
                ],
            ),
            affine(
                context.parse("-m2").unwrap(),
                [
                    one.clone(),
                    context.integer(2),
                    one,
                    zero.clone(),
                    zero.clone(),
                ],
            ),
        ],
        vec![vec![context.parameter("s").unwrap()]],
        (0..3)
            .map(|index| context.parameter(&format!("nu{index}")).unwrap())
            .collect(),
    )
    .unwrap();

    assert_eq!(completion.schema(), "rustred-automatic-isp-completion-v1");
    assert_eq!(completion.input_denominator_count(), 3);
    assert_eq!(completion.appended_coordinate_ordinals(), &[3, 4]);
    assert_eq!(
        completion.appended_coordinates().collect::<Vec<_>>(),
        vec![
            ScalarProductCoordinate::LoopExternal {
                loop_index: 0,
                external_index: 0,
            },
            ScalarProductCoordinate::LoopExternal {
                loop_index: 1,
                external_index: 0,
            },
        ]
    );
    assert_eq!(completion.rank_progression(), &[3, 4, 5]);
    assert_eq!(completion.stats().appended_isps(), 2);
    assert!(completion.stats().rank_tests() >= 6);
    assert!(completion.stats().rank_operations() > 0);
    assert!(completion.family().power_shifts()[3].is_zero());
    assert!(completion.family().power_shifts()[4].is_zero());
    completion.replay().unwrap();

    // The completed family immediately drives the generic generator.  There
    // is no topology or loop-count dispatch in either path.
    let generated = ParametricIbpGenerator::try_new(completion.family())
        .unwrap()
        .generate()
        .unwrap();
    assert_eq!(generated.ordinary_ibp().len(), 6);
    assert!(generated.lorentz_invariance().is_empty());
}

#[test]
fn rank_increasing_unit_scan_is_deterministic_in_rustred_order() {
    let context = CoefficientContext::new(["d", "m", "s"]);
    let completion = AutomaticIspCompletion::try_new(
        "automatic-isp-unit-order",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![affine(
            context.parse("-m").unwrap(),
            [context.one(), context.one()],
        )],
        vec![vec![context.parameter("s").unwrap()]],
        vec![context.zero()],
    )
    .unwrap();

    // For the physical row [1,1], e0 is the first rank-increasing identity
    // row under RustRed's persisted coordinate order. Mathematica `Union` can
    // order symbolic scalar products differently, yielding an equivalent
    // complete ISP basis with different ordinals.
    assert_eq!(completion.appended_coordinate_ordinals(), &[0]);
    assert_eq!(
        completion.appended_coordinates().collect::<Vec<_>>(),
        vec![ScalarProductCoordinate::LoopLoop { left: 0, right: 0 }]
    );
    assert_eq!(completion.rank_progression(), &[1, 2]);
    completion.replay().unwrap();
}

#[test]
fn dependent_inputs_are_not_relabelled_as_isps() {
    let context = CoefficientContext::new(["d", "m", "s"]);
    let row = affine(context.parse("-m").unwrap(), [context.one(), context.one()]);
    let error = AutomaticIspCompletion::try_new(
        "automatic-isp-dependent",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![row.clone(), row],
        vec![vec![context.parameter("s").unwrap()]],
        vec![context.zero(), context.zero()],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        AutomaticIspCompletionError::DependentInputDenominators {
            denominators: 2,
            generic_rank: 1,
        }
    ));
}

#[test]
fn rank_work_is_preflighted_by_typed_limits() {
    let context = CoefficientContext::new(["d", "m"]);
    let mut limits = AutomaticIspCompletionLimits::default();
    limits.max_rank_operations = 0;
    let error = AutomaticIspCompletion::try_new_with_limits(
        "automatic-isp-limited",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![affine(context.parse("-m").unwrap(), [context.one()])],
        Vec::new(),
        vec![context.zero()],
        limits,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        AutomaticIspCompletionError::ResourceLimit {
            resource: "automatic ISP rank operations",
            requested: 1,
            limit: 0,
        }
    ));
}

#[test]
fn supplied_rank_matrix_is_bounded_before_the_internal_row_clone() {
    let context = CoefficientContext::new(["d", "m", "s"]);
    let mut limits = AutomaticIspCompletionLimits::default();
    limits.max_rank_matrix_entries = 1;
    let error = AutomaticIspCompletion::try_new_with_limits(
        "automatic-isp-initial-matrix-limited",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![affine(
            context.parse("-m").unwrap(),
            [context.one(), context.one()],
        )],
        vec![vec![context.parameter("s").unwrap()]],
        vec![context.zero()],
        limits,
    )
    .unwrap_err();
    assert_eq!(
        error,
        AutomaticIspCompletionError::ResourceLimit {
            resource: "automatic ISP rank matrix entries",
            requested: 2,
            limit: 1,
        }
    );
}

#[test]
fn candidate_matrix_payload_is_bounded_before_candidate_allocation() {
    let context = CoefficientContext::new(["d", "m", "s"]);
    let mut limits = AutomaticIspCompletionLimits::default();
    // [1,0] owns three polynomial terms (one numerator plus two unit
    // denominators). The first two-row scout matrix owns six.
    limits.max_rank_coefficient_terms = 5;
    let error = AutomaticIspCompletion::try_new_with_limits(
        "automatic-isp-candidate-payload-limited",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![affine(
            context.parse("-m").unwrap(),
            [context.one(), context.zero()],
        )],
        vec![vec![context.parameter("s").unwrap()]],
        vec![context.zero()],
        limits,
    )
    .unwrap_err();
    assert_eq!(
        error,
        AutomaticIspCompletionError::ResourceLimit {
            resource: "automatic ISP rank coefficient terms",
            requested: 6,
            limit: 5,
        }
    );
}

#[test]
fn foreign_symbolica_coefficient_map_is_rejected_before_rank_arithmetic() {
    let context = CoefficientContext::new(["d", "m"]);
    let foreign = CoefficientContext::new(["d", "x"]);
    let error = AutomaticIspCompletion::try_new(
        "automatic-isp-foreign-coefficient",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![affine(context.zero(), [foreign.one()])],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        AutomaticIspCompletionError::InvalidInputCoefficient {
            denominator: 0,
            coordinate: Some(0),
            error: ExactAlgebraError::VariableMapMismatch { .. },
        }
    ));
}

#[test]
fn coefficient_denominator_domain_provenance_survives_completion_and_replay() {
    let context = CoefficientContext::new(["d", "x"]);
    let completion = AutomaticIspCompletion::try_new(
        "automatic-isp-rational-domain",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![affine(context.zero(), [context.parse("1/x").unwrap()])],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap();

    let condition = completion
        .family()
        .domain()
        .input_denominators()
        .iter()
        .find(|condition| condition.polynomial().to_expression().to_string() == "x")
        .expect("the uncancelled input denominator x must remain explicit");
    assert!(condition.origins().iter().any(|origin| matches!(
        origin,
        GuardOrigin::FamilyInputCoefficientDenominator { .. }
    )));
    completion.replay().unwrap();
}
