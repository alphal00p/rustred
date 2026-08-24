use std::sync::Arc;

use rustred::{
    CoefficientContext, IndexShift, IntegralOrderingPolicy, ParametricCoefficient,
    ParametricCoefficientContext, ParametricElimination, ParametricEliminationLimits,
    ParametricEliminationOrdering, ParametricReductionRuleCandidate, ParametricRelation,
    ParametricRowId, ParametricRuleApplication, ParametricRuleLimits, SectorMask,
    SymbolicSectorCaseError, WhenBadBoundaryHazardKind, WhenBadCompilation, WhenBadCompiler,
    WhenBadCompilerError, WhenBadCompilerLimits, WhenBadDescentComponent, WhenBadLeafDisposition,
    WhenBadSourceAuthentication, WhenBadUnsupportedReason,
};

struct Fixture {
    context: ParametricCoefficientContext,
    candidate: ParametricReductionRuleCandidate,
}

fn synthetic_candidate(
    scope: &str,
    sector: SectorMask,
    anchor: &[i64],
    rhs_shift: &[i64],
    coefficient: impl FnOnce(&ParametricCoefficientContext) -> ParametricCoefficient,
    decorate: impl FnOnce(&ParametricCoefficientContext, &mut ParametricRelation),
) -> Fixture {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = ParametricCoefficientContext::try_new(&base, scope, sector.arity()).unwrap();
    let mut row = ParametricRelation::new(
        format!("adversarial-fabricated-family-{scope}"),
        ParametricRowId::Derived {
            label: Arc::from(format!("adversarial-fabricated-row-{scope}")),
        },
        &context,
    );
    let zero = IndexShift::try_new(vec![0; sector.arity()], sector.arity()).unwrap();
    row.add_term(&context, zero.clone(), context.one()).unwrap();
    row.add_term(
        &context,
        IndexShift::try_new(rhs_shift.iter().copied(), sector.arity()).unwrap(),
        coefficient(&context),
    )
    .unwrap();
    decorate(&context, &mut row);

    let rows = vec![row];
    let elimination = ParametricElimination::build(
        &context,
        &rows,
        ParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            anchor.iter().copied(),
        )
        .unwrap(),
        ParametricEliminationLimits::default(),
    )
    .unwrap();
    let pivot = elimination
        .pivots()
        .iter()
        .position(|pivot| pivot.pivot() == &zero)
        .expect("the requested anchor must make the zero shift the solved pivot");
    let candidate = ParametricReductionRuleCandidate::try_from_elimination_pivot(
        &context,
        &rows,
        &elimination,
        pivot,
        sector,
        ParametricRuleLimits::default(),
    )
    .unwrap();
    Fixture { context, candidate }
}

fn compile(fixture: &Fixture) -> rustred::WhenBadCertificate {
    let WhenBadCompilation::Certified(certificate) = WhenBadCompiler::compile_algebraic_candidate(
        &fixture.context,
        &fixture.candidate,
        WhenBadCompilerLimits::default(),
    )
    .unwrap() else {
        panic!("the synthetic descending fixture must be certifiable")
    };
    certificate
}

fn disposition(fixture: &Fixture, indices: &[i64]) -> WhenBadLeafDisposition {
    compile(fixture)
        .classification_for_indices(&fixture.context, indices)
        .unwrap()
        .expect("the point belongs to the fixture orthant")
        .disposition()
        .clone()
}

#[test]
fn fabricated_rows_are_accepted_only_with_the_explicit_algebraic_marker() {
    let fixture = synthetic_candidate(
        "algebraic-only-marker",
        SectorMask::try_new([false]).unwrap(),
        &[-2],
        &[1],
        |context| context.one(),
        |_, _| {},
    );
    let certificate = compile(&fixture);

    assert_eq!(
        certificate.binding().source_authentication(),
        WhenBadSourceAuthentication::AlgebraicOnly
    );
    assert!(
        certificate
            .binding()
            .family_fingerprint()
            .contains("fabricated-family")
    );
    certificate.replay(&fixture.context).unwrap();
}

#[test]
fn two_inactive_activation_hazards_are_an_or_with_exact_zero_gates() {
    let fixture = synthetic_candidate(
        "two-coordinate-or",
        SectorMask::try_new([false, false]).unwrap(),
        &[-2, -2],
        &[1, 1],
        |context| {
            context
                .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                .unwrap()
        },
        |_, _| {},
    );
    let certificate = compile(&fixture);
    assert_eq!(certificate.stats().boundary_values_examined(), 2);
    assert_eq!(certificate.stats().leak_events(), 2);

    // Each single boundary activates one inactive line with a nonzero
    // coefficient and is therefore exceptional.
    assert!(matches!(
        disposition(&fixture, &[0, -1]),
        WhenBadLeafDisposition::ExceptionalSectorLeak { .. }
    ));
    assert!(matches!(
        disposition(&fixture, &[-1, 0]),
        WhenBadLeafDisposition::ExceptionalSectorLeak { .. }
    ));

    // At the intersection the shared numerator n0+n1 vanishes. Both leak
    // events are removed; this is the sharp Symbolica refinement of WhenBad.
    assert_eq!(
        disposition(&fixture, &[0, 0]),
        WhenBadLeafDisposition::CoveredByCandidate
    );
    assert_eq!(
        disposition(&fixture, &[-1, -1]),
        WhenBadLeafDisposition::CoveredByCandidate
    );
    certificate.replay(&fixture.context).unwrap();
}

#[test]
fn mixed_uniform_descent_splits_the_finite_i64_overflow_edge_exactly() {
    // The exact corner-distance delta is uniformly -1.  The +1 first
    // component is therefore allowed away from the one concrete i64 edge
    // point, which is retained as its own exceptional leaf.
    let fixture = synthetic_candidate(
        "outward-overflow",
        SectorMask::try_new([true, true]).unwrap(),
        &[1, 3],
        &[1, -2],
        |context| context.one(),
        |_, _| {},
    );
    let certificate = compile(&fixture);
    assert_eq!(certificate.stats().boundary_values_examined(), 1);
    assert_eq!(certificate.stats().leak_events(), 1);
    assert_eq!(
        certificate.leak_events()[0].kind(),
        WhenBadBoundaryHazardKind::ConcreteIndexOverflow,
    );
    assert_eq!(certificate.leak_events()[0].coordinate(), 0);
    assert_eq!(
        disposition(&fixture, &[1, 3]),
        WhenBadLeafDisposition::CoveredByCandidate,
    );
    assert!(matches!(
        disposition(&fixture, &[i64::MAX, 3]),
        WhenBadLeafDisposition::ExceptionalSectorLeak { .. }
    ));
    assert!(
        fixture
            .candidate
            .apply(&fixture.context, &[i64::MAX, 3])
            .is_err()
    );
    certificate.replay(&fixture.context).unwrap();
}

#[test]
fn aggregate_ties_use_the_exact_persisted_lexicographic_components() {
    // The corner-distance delta vanishes: (-1) on the active line and (+1)
    // on the inactive line cancel.  Dot power is therefore decisive.
    let dot_tie = synthetic_candidate(
        "dot-power-tie",
        SectorMask::try_new([true, false]).unwrap(),
        &[2, -1],
        &[-1, -1],
        |context| context.one(),
        |_, _| {},
    );
    let certificate = compile(&dot_tie);
    assert_eq!(
        certificate.descent_witnesses()[0].decisive_component(),
        WhenBadDescentComponent::DotPower,
    );
    assert_eq!(
        IntegralOrderingPolicy::RustRedUnshiftedV1
            .compare(&[1, -2], &[2, -1])
            .unwrap(),
        std::cmp::Ordering::Less,
    );
    assert_eq!(
        disposition(&dot_tie, &[2, i64::MIN + 1]),
        WhenBadLeafDisposition::CoveredByCandidate,
    );
    assert!(matches!(
        disposition(&dot_tie, &[2, i64::MIN]),
        WhenBadLeafDisposition::ExceptionalSectorLeak { .. }
    ));

    // All three aggregate components tie.  The first per-index excess is
    // negative, so [-1,+1] is still a strict same-sector descent.
    let index_tie = synthetic_candidate(
        "index-excess-tie",
        SectorMask::try_new([true, true]).unwrap(),
        &[2, 1],
        &[-1, 1],
        |context| context.one(),
        |_, _| {},
    );
    let certificate = compile(&index_tie);
    assert_eq!(
        certificate.descent_witnesses()[0].decisive_component(),
        WhenBadDescentComponent::IndexExcess { position: 0 },
    );
    assert_eq!(
        IntegralOrderingPolicy::RustRedUnshiftedV1
            .compare(&[1, 2], &[2, 1])
            .unwrap(),
        std::cmp::Ordering::Less,
    );

    // Reversing the tied per-index deltas is a strict ascent.  An anchor in
    // another orthant can still select the fabricated zero-shift pivot, but
    // WhenBad must reject it for the requested active sector.
    let reverse = synthetic_candidate(
        "index-excess-tie-ascent",
        SectorMask::try_new([true, true]).unwrap(),
        &[-2, 2],
        &[1, -1],
        |context| context.one(),
        |_, _| {},
    );
    let WhenBadCompilation::Unsupported(unsupported) =
        WhenBadCompiler::compile_algebraic_candidate(
            &reverse.context,
            &reverse.candidate,
            WhenBadCompilerLimits::default(),
        )
        .unwrap()
    else {
        panic!("the reversed index-excess tie must be rejected")
    };
    assert!(matches!(
        unsupported.reason(),
        WhenBadUnsupportedReason::NonUniformSameSectorDescent {
            first_nonzero_component: WhenBadDescentComponent::IndexExcess { position: 0 },
            delta: 1,
            ..
        }
    ));
    unsupported.replay(&reverse.context).unwrap();
}

#[test]
fn exact_multi_point_hazard_ranges_cover_overflow_underflow_and_activation() {
    let positive_overflow = synthetic_candidate(
        "positive-overflow-range",
        SectorMask::try_new([true, true]).unwrap(),
        &[1, 5],
        &[3, -4],
        |context| context.one(),
        |_, _| {},
    );
    let certificate = compile(&positive_overflow);
    assert_eq!(certificate.stats().boundary_values_examined(), 3);
    assert_eq!(certificate.stats().leak_events(), 3);
    assert!(certificate.leak_events().iter().all(|event| {
        event.kind() == WhenBadBoundaryHazardKind::ConcreteIndexOverflow && event.coordinate() == 0
    }));
    assert_eq!(
        certificate
            .leak_events()
            .iter()
            .map(|event| event.boundary_value())
            .collect::<Vec<_>>(),
        vec![i64::MAX - 2, i64::MAX - 1, i64::MAX],
    );
    assert_eq!(
        disposition(&positive_overflow, &[i64::MAX - 3, 5]),
        WhenBadLeafDisposition::CoveredByCandidate,
    );

    let negative_underflow = synthetic_candidate(
        "negative-underflow-range",
        SectorMask::try_new([true, false]).unwrap(),
        &[5, -1],
        &[-4, -3],
        |context| context.one(),
        |_, _| {},
    );
    let certificate = compile(&negative_underflow);
    assert_eq!(certificate.stats().boundary_values_examined(), 3);
    assert_eq!(
        certificate
            .leak_events()
            .iter()
            .map(|event| event.boundary_value())
            .collect::<Vec<_>>(),
        vec![i64::MIN, i64::MIN + 1, i64::MIN + 2],
    );
    assert!(certificate.leak_events().iter().all(|event| {
        event.kind() == WhenBadBoundaryHazardKind::ConcreteIndexOverflow && event.coordinate() == 1
    }));
    assert_eq!(
        disposition(&negative_underflow, &[5, i64::MIN + 3]),
        WhenBadLeafDisposition::CoveredByCandidate,
    );

    let inactive_activation = synthetic_candidate(
        "inactive-activation-range",
        SectorMask::try_new([true, false]).unwrap(),
        &[5, -3],
        &[-4, 3],
        |context| context.one(),
        |_, _| {},
    );
    let certificate = compile(&inactive_activation);
    assert_eq!(
        certificate
            .leak_events()
            .iter()
            .map(|event| event.boundary_value())
            .collect::<Vec<_>>(),
        vec![-2, -1, 0],
    );
    assert!(certificate.leak_events().iter().all(|event| {
        event.kind() == WhenBadBoundaryHazardKind::InactiveSectorActivation
            && event.coordinate() == 1
    }));
    assert_eq!(
        disposition(&inactive_activation, &[5, -3]),
        WhenBadLeafDisposition::CoveredByCandidate,
    );
    for boundary in [-2, -1, 0] {
        assert!(matches!(
            disposition(&inactive_activation, &[5, boundary]),
            WhenBadLeafDisposition::ExceptionalSectorLeak { .. }
        ));
    }
}

#[test]
fn zero_coefficients_at_both_integer_extremes_precede_key_arithmetic() {
    let positive = synthetic_candidate(
        "zero-at-positive-overflow",
        SectorMask::try_new([true, true]).unwrap(),
        &[1, 3],
        &[1, -2],
        |context| {
            context
                .sub(&context.index(0).unwrap(), &context.integer(i64::MAX))
                .unwrap()
        },
        |_, _| {},
    );
    let certificate = compile(&positive);
    assert_eq!(certificate.stats().boundary_values_examined(), 1);
    assert_eq!(certificate.stats().leak_events(), 0);
    assert_eq!(
        disposition(&positive, &[i64::MAX, 3]),
        WhenBadLeafDisposition::CoveredByCandidate,
    );
    let ParametricRuleApplication::Applicable(reduction) = positive
        .candidate
        .apply(&positive.context, &[i64::MAX, 3])
        .unwrap()
    else {
        panic!("a zero overflow term must be discarded before key addition")
    };
    assert!(reduction.rhs().is_empty());
    certificate.replay(&positive.context).unwrap();

    let negative = synthetic_candidate(
        "zero-at-negative-underflow",
        SectorMask::try_new([true, false]).unwrap(),
        &[3, -1],
        &[-2, -1],
        |context| {
            context
                .sub(&context.index(1).unwrap(), &context.integer(i64::MIN))
                .unwrap()
        },
        |_, _| {},
    );
    let certificate = compile(&negative);
    assert_eq!(certificate.stats().boundary_values_examined(), 1);
    assert_eq!(certificate.stats().leak_events(), 0);
    assert_eq!(
        disposition(&negative, &[3, i64::MIN]),
        WhenBadLeafDisposition::CoveredByCandidate,
    );
    let ParametricRuleApplication::Applicable(reduction) = negative
        .candidate
        .apply(&negative.context, &[3, i64::MIN])
        .unwrap()
    else {
        panic!("a zero underflow term must be discarded before key addition")
    };
    assert!(reduction.rhs().is_empty());
    certificate.replay(&negative.context).unwrap();
}

#[test]
fn extreme_fixed_shifts_are_either_safe_or_rejected_by_preflight_limits() {
    // An active negative MIN shift never underflows because every source is
    // at least one.  Both endpoint applications remain representable.
    let active_negative = synthetic_candidate(
        "active-negative-min",
        SectorMask::try_new([true]).unwrap(),
        &[1],
        &[i64::MIN],
        |context| context.one(),
        |_, _| {},
    );
    let certificate = compile(&active_negative);
    assert_eq!(certificate.stats().boundary_values_examined(), 0);
    assert!(matches!(
        active_negative
            .candidate
            .apply(&active_negative.context, &[1])
            .unwrap(),
        ParametricRuleApplication::Applicable(_)
    ));
    assert!(matches!(
        active_negative
            .candidate
            .apply(&active_negative.context, &[i64::MAX])
            .unwrap(),
        ParametricRuleApplication::Applicable(_)
    ));

    // MAX on an active line is made descending by MIN on another active
    // line.  The proof is sound, but its MAX-sized overflow interval must be
    // rejected before any boundary polynomial is allocated.
    let active_positive = synthetic_candidate(
        "active-positive-max-budget",
        SectorMask::try_new([true, true]).unwrap(),
        &[i64::MIN, 1],
        &[i64::MAX, i64::MIN],
        |context| context.one(),
        |_, _| {},
    );
    assert!(matches!(
        WhenBadCompiler::compile_algebraic_candidate(
            &active_positive.context,
            &active_positive.candidate,
            WhenBadCompilerLimits::default(),
        ),
        Err(WhenBadCompilerError::ResourceLimit {
            resource: "WhenBad boundary values per RHS",
            requested,
            limit: 1_000_000,
        }) if requested == usize::try_from(i64::MAX.unsigned_abs()).unwrap()
    ));

    // MIN on an inactive line has exactly 2^63 underflow points.  A matching
    // active MIN shift makes the aggregate order descending, so the same
    // preflight—not the descent check—must stop the compilation.
    let inactive_negative = synthetic_candidate(
        "inactive-negative-min-budget",
        SectorMask::try_new([true, false]).unwrap(),
        &[1, 0],
        &[i64::MIN, i64::MIN],
        |context| context.one(),
        |_, _| {},
    );
    assert!(matches!(
        WhenBadCompiler::compile_algebraic_candidate(
            &inactive_negative.context,
            &inactive_negative.candidate,
            WhenBadCompilerLimits::default(),
        ),
        Err(WhenBadCompilerError::ResourceLimit {
            resource: "WhenBad boundary values per RHS",
            requested,
            limit: 1_000_000,
        }) if requested == usize::try_from(i64::MIN.unsigned_abs()).unwrap()
    ));
}

#[test]
fn aggregate_boundary_budget_is_checked_before_event_enumeration() {
    let fixture = synthetic_candidate(
        "aggregate-boundary-budget",
        SectorMask::try_new([true, true]).unwrap(),
        &[2, 3],
        &[1, -2],
        |context| context.one(),
        |context, row| {
            row.add_term(
                context,
                IndexShift::try_new([-2, 1], 2).unwrap(),
                context.one(),
            )
            .unwrap();
        },
    );
    let mut limits = WhenBadCompilerLimits::default();
    limits.max_boundary_values_per_rhs = 1;
    limits.max_boundary_values = 1;
    assert!(matches!(
        WhenBadCompiler::compile_algebraic_candidate(&fixture.context, &fixture.candidate, limits,),
        Err(WhenBadCompilerError::ResourceLimit {
            resource: "WhenBad boundary values",
            requested: 2,
            limit: 1,
        })
    ));
    fixture.candidate.replay_retained(&fixture.context).unwrap();
}

#[test]
fn aggregate_limits_fail_closed_and_do_not_damage_the_replayable_candidate() {
    let fixture = synthetic_candidate(
        "aggregate-limits",
        SectorMask::try_new([true]).unwrap(),
        &[3],
        &[-1],
        |context| context.one(),
        |context, row| {
            for constant in [1, 2] {
                let guard = context
                    .sub(&context.index(0).unwrap(), &context.integer(constant))
                    .unwrap();
                row.add_nonzero_condition(context, context.numerator_condition(&guard).unwrap())
                    .unwrap();
            }
        },
    );
    let baseline = compile(&fixture);
    let stats = baseline.stats();
    assert!(stats.domain_condition_sources() >= 2);
    assert!(stats.guard_origins() >= 2);
    assert!(stats.retained_condition_terms() >= 2);
    assert!(stats.retained_condition_bytes() >= 2);
    assert!(stats.leaf_classifications() >= 3);

    let mut limits = WhenBadCompilerLimits::default();
    limits.max_domain_condition_sources = stats.domain_condition_sources() - 1;
    assert!(matches!(
        WhenBadCompiler::compile_algebraic_candidate(&fixture.context, &fixture.candidate, limits,),
        Err(WhenBadCompilerError::ResourceLimit {
            resource: "WhenBad domain condition sources",
            ..
        })
    ));

    limits = WhenBadCompilerLimits::default();
    limits.max_guard_origins = stats.guard_origins() - 1;
    assert!(matches!(
        WhenBadCompiler::compile_algebraic_candidate(&fixture.context, &fixture.candidate, limits,),
        Err(WhenBadCompilerError::ResourceLimit {
            resource: "WhenBad guard origins",
            ..
        })
    ));

    limits = WhenBadCompilerLimits::default();
    limits.max_retained_condition_terms = stats.retained_condition_terms() - 1;
    assert!(matches!(
        WhenBadCompiler::compile_algebraic_candidate(&fixture.context, &fixture.candidate, limits,),
        Err(WhenBadCompilerError::ResourceLimit {
            resource: "WhenBad retained condition terms",
            ..
        })
    ));

    limits = WhenBadCompilerLimits::default();
    limits.max_retained_condition_bytes = stats.retained_condition_bytes() - 1;
    assert!(matches!(
        WhenBadCompiler::compile_algebraic_candidate(&fixture.context, &fixture.candidate, limits,),
        Err(WhenBadCompilerError::ResourceLimit {
            resource: "WhenBad retained condition bytes",
            ..
        })
    ));

    limits = WhenBadCompilerLimits::default();
    limits.max_leaf_classifications = stats.leaf_classifications() - 1;
    assert!(matches!(
        WhenBadCompiler::compile_algebraic_candidate(&fixture.context, &fixture.candidate, limits,),
        Err(WhenBadCompilerError::SectorCase(
            SymbolicSectorCaseError::ResourceLimit {
                resource: "live symbolic sector cases",
                ..
            }
        ))
    ));

    // Every compilation failure is transactional with respect to the input.
    fixture.candidate.replay_retained(&fixture.context).unwrap();
    baseline.replay(&fixture.context).unwrap();
    compile(&fixture).replay(&fixture.context).unwrap();
}
