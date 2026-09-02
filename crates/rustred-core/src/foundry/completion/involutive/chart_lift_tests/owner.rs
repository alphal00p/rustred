#[test]
fn mixed_active_inactive_source_gets_the_minimal_exact_left_lift() {
    let limits = OrdinaryChartLiftLimits::default();
    let context = context("ordinary-chart-lift-mixed", 2);
    let ordering = ordering(&[true, false], limits.involutive);
    let relation = build_mixed_relation(&context, false);
    let lifted = lift_relation(&relation, 4, &ordering, &context, limits).unwrap();

    // Physical displacements (-2,+1) and (+1,-3) become chart
    // displacements (-2,-1) and (+1,+3).  Their minimal common left
    // shift is therefore (+2,+1), producing (0,0) and (3,4).
    assert_eq!(lifted.left_shift().values(), &[2, 1]);
    assert_eq!(lifted.row().terms().len(), 2);
    let zero = shift(&[0, 0], limits.involutive);
    let far = shift(&[3, 4], limits.involutive);

    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let expected_first_numerator = context
        .add(&context.add(&n0, &n1).unwrap(), &context.one())
        .unwrap();
    let expected_first_denominator = context.add(&n1, &context.one()).unwrap();
    let expected_first = context
        .div(&expected_first_numerator, &expected_first_denominator)
        .unwrap();
    let expected_second = context
        .add(&context.sub(&n0, &n1).unwrap(), &context.integer(3))
        .unwrap();
    assert_eq!(lifted.row().coefficient(&zero), Some(&expected_first));
    assert_eq!(lifted.row().coefficient(&far), Some(&expected_second));

    let provenance = lifted.consequence.provenance();
    assert_eq!(provenance.terms().len(), 1);
    assert_eq!(provenance.terms()[0].source_ordinal(), 4);
    assert_eq!(provenance.terms()[0].left_shift().values(), &[2, 1]);
    assert_eq!(provenance.terms()[0].left_coefficient(), &context.one());

    // The input denominator n1+2 is acted on by the inactive physical
    // translation n1 -> n1-1 and is retained as the exact guard n1+1.
    let expected_guard = context
        .numerator_condition_with_limits(
            &expected_first_denominator,
            limits.involutive.indexed_algebra.exact_algebra,
        )
        .unwrap();
    assert_eq!(lifted.consequence.required_nonzero_guards().len(), 1);
    assert_eq!(
        lifted.consequence.required_nonzero_guards()[0].as_ref(),
        &expected_guard
    );
}

#[test]
fn sealed_source_provenance_replays_exactly_against_its_owner_only() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = OrdinaryChartLiftLimits::default();
    let adapter = completed_ordering(&[true, false, true], &completed, limits.involutive);
    let forged_ordinal = completed.source_row_count();
    let forged_row = OreRow::try_new(
        &adapter,
        [(
            shift(&[0, 0, 0], limits.involutive),
            generator.context().one(),
        )],
        generator.context(),
        limits.involutive,
    )
    .unwrap();
    assert_eq!(
        OreConsequence::try_from_left_shifted_source(
            forged_ordinal,
            shift(&[0, 0, 0], limits.involutive),
            forged_row,
            &adapter,
            generator.context(),
            limits.involutive,
        ),
        Err(InvolutiveError::SourceOrdinalOutOfRange {
            source_ordinal: forged_ordinal,
            source_count: completed.source_row_count(),
        })
    );
    let lifted =
        try_lift_completed_ordinary_sources(&completed, &adapter, generator.context(), limits)
            .unwrap();
    assert_eq!(lifted.len(), 4);
    assert!(!lifted.is_empty());

    for (source_ordinal, retained) in lifted.sources().iter().enumerate() {
        let replayed = lifted
            .try_replay_source(
                source_ordinal,
                &completed,
                &adapter,
                generator.context(),
                limits,
            )
            .unwrap();
        assert_eq!(&replayed, retained);
        let provenance = retained.consequence.provenance();
        assert_eq!(provenance.terms().len(), 1);
        assert_eq!(provenance.terms()[0].source_ordinal(), source_ordinal);
        assert_eq!(provenance.terms()[0].left_shift(), retained.left_shift());
    }

    let foreign_ordering = ordering(&[true, false, true], limits.involutive);
    assert_eq!(
        lifted.try_replay_source(
            0,
            &completed,
            &foreign_ordering,
            generator.context(),
            limits,
        ),
        Err(OrdinaryChartLiftError::ForeignSourceOwner)
    );

    let foreign = complete_ordinary(&generator);
    let foreign_barrier_ordering =
        completed_ordering(&[true, false, true], &foreign, limits.involutive);
    assert_eq!(
        lifted.try_replay_source(
            0,
            &foreign,
            &foreign_barrier_ordering,
            generator.context(),
            limits,
        ),
        Err(OrdinaryChartLiftError::ForeignSourceOwner)
    );

    let rejected =
        try_lift_completed_ordinary_sources(&completed, &adapter, generator.context(), limits)
            .unwrap();
    assert!(matches!(
        rejected.try_into_consequences(
            &foreign,
            &foreign_barrier_ordering,
            generator.context(),
            limits.involutive,
        ),
        Err(OrdinaryChartLiftError::ForeignSourceOwner)
    ));

    let accepted =
        try_lift_completed_ordinary_sources(&completed, &adapter, generator.context(), limits)
            .unwrap()
            .try_into_consequences(&completed, &adapter, generator.context(), limits.involutive)
            .unwrap();
    let epoch = JanetBasisEpoch::try_initial(
        accepted.into_vec().into_iter().take(1),
        &adapter,
        generator.context(),
        limits.involutive,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(epoch.require_ordering(&adapter), Ok(()));
    assert_eq!(
        epoch.require_ordering(&foreign_barrier_ordering),
        Err(InvolutiveError::ForeignOreAction)
    );
}

#[test]
fn external_only_and_foreign_context_sources_are_rejected_at_scope_ingress() {
    let base = CoefficientContext::new(["d", "a", "b", "s", "g"]);
    let family = IntegralFamily::new(
        "ordinary-chart-lift-external-only",
        vec!["k".into()],
        vec!["p".into()],
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                base.zero(),
                vec![base.coefficient_fixture("a/s"), base.one()],
            ),
            AffineDenominator::new(
                base.zero(),
                vec![base.parameter("b").unwrap(), base.integer(2)],
            ),
        ],
        vec![vec![base.parameter("g").unwrap()]],
        vec![base.zero(), base.zero()],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let prepared = generator.prepare_external_ibp_sources().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let external = prepared.complete(rows).unwrap();
    let limits = OrdinaryChartLiftLimits::default();
    let external_ordering = ordering(&[true, true], limits.involutive);
    assert_eq!(
        try_lift_completed_ordinary_sources(
            &external,
            &external_ordering,
            generator.context(),
            limits,
        ),
        Err(OrdinaryChartLiftError::SourceLayout {
            actual: "external-contraction IBP source"
        })
    );

    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let foreign = context("ordinary-chart-lift-foreign", 3);
    let ordinary_ordering = ordering(&[true, true, true], limits.involutive);
    assert_eq!(
        try_lift_completed_ordinary_sources(&completed, &ordinary_ordering, &foreign, limits,),
        Err(OrdinaryChartLiftError::ContextMismatch)
    );
}
