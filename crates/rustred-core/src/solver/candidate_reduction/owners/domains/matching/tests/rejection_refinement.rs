//! A speculative native preflight refusal preserves the ORIGINAL guard's
//! finite refinement or Unknown, never applicability or a later-rule bypass.
use super::*;

fn limits(faces: usize) -> OwnerDomainMatchLimits {
    let mut limits = OwnerDomainMatchLimits {
        max_bounded_refinement_cells: faces,
        ..Default::default()
    };
    // Original bilinear/affine two-variable support costs128. The broad later
    // quadratic costs2592; its specialized univariate lane fits this cap.
    limits.guard_algebra.max_gcd_factor_work = 1000;
    limits
}
fn costly(c: &IndexedCoefficientContext, poles: bool) -> IndexedPolynomial {
    let a = c.mul(&c.index(0).unwrap(), &c.index(0).unwrap()).unwrap();
    let b = c.mul(&c.index(1).unwrap(), &c.index(1).unwrap()).unwrap();
    poly(
        c,
        if poles {
            c.sub(&a, &b).unwrap()
        } else {
            c.add(&c.add(&a, &b).unwrap(), &c.one()).unwrap()
        },
    )
}
fn retry_fixture(poles: bool) -> Arc<CandidateOwnerPrograms<3>> {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut first = rule(17);
    // Only the ORIGINAL guard involves finite inactive axis2. The later pole
    // has only unbounded positive axes0/1: refining its support cannot work.
    first.equalities.push(poly(
        &c,
        c.add(&c.index(0).unwrap(), &c.index(2).unwrap()).unwrap(),
    ));
    first.rhs.push(PreparedTerm {
        shift: [0; 3],
        coefficient: c.zero(),
        denominator: costly(&c, poles),
    });
    install(&mut p, first);
    p
}
fn later() -> OwnerDomainPredicate {
    OwnerDomainPredicate::OriginalDenominator {
        batch: 0,
        rule: 17,
        term: 0,
    }
}
fn failure() -> OwnerDomainMatchFailure {
    OwnerDomainMatchFailure::Algebra(crate::algebra::IndexedAlgebraError::ResourceLimit {
        resource: "guard separable factor work",
        requested: 2592,
        limit: 1000,
    })
}
fn refuse(
    p: &CandidateOwnerPrograms<3>,
    rank: Option<u32>,
    limits: OwnerDomainMatchLimits,
) -> OwnerDomainMatchError {
    p.visit_owner_domain_matches(
        OWNER,
        &[0; 3],
        &[None; 3],
        rank,
        limits,
        &AtomicBool::new(false),
        |_| panic!("refusal cannot publish a partial split"),
    )
    .unwrap_err()
}

#[test]
fn rejection_refinement_uses_original_support_phase_and_rechecks_actual_poles() {
    for poles in [false, true] {
        let p = retry_fixture(poles);
        let (no_split, unknown) = refined(&p, [0; 3], [None; 3], Some(2), limits(0));
        assert_eq!(no_split.predicates, 2);
        assert_eq!(unknown.len(), 1);
        assert_eq!(
            unknown[0].disposition(),
            OwnerDomainMatchDisposition::Unresolved {
                predicate: equality(0),
            }
        );
        let (stats, automatic) = refined(&p, [0; 3], [None; 3], Some(2), limits(3));
        let mut explicit = Vec::new();
        let mut explicit_predicates = 0;
        for k in 0..=2 {
            let (s, pieces) = refined(&p, [0, 0, k], [None, None, Some(k)], Some(2), limits(0));
            explicit_predicates += s.predicates;
            explicit.extend(pieces);
        }
        assert_eq!(normalized(&automatic), normalized(&explicit));
        assert_eq!((stats.refinement_cells, stats.refinement_steps), (3, 1));
        assert_eq!(stats.predicates, explicit_predicates + 2); // neither attempt refunded
        parity(&p, &automatic, 2);
        assert_eq!(
            at(&automatic, [0, 0, 1]),
            if poles {
                selected()
            } else {
                OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 17 }
            }
        );
        assert!(automatic.iter().any(|piece| piece.upper()[0].is_none()));
    }
}

#[test]
fn rejection_refinement_actual_fifteen_point_shape_keeps_all_previously_applicable_cells() {
    // Actual failed geometry after dropping other fixed coordinates: all three
    // coordinates are inactive; x0>=4,x1>=1,x2=1 and x0+x1+x2<=10 gives15points.
    // The original dynamic Unknown polynomial was not logged. This is a small
    // symbolic representative, not an exact saved-rule/polynomial replay.
    const FINITE_OWNER: [bool; 3] = [false; 3];
    let mut p = programs(
        Arc::new(crate::solver::tests::sunset()),
        Some(10),
        vec![input(FINITE_OWNER, Some(10), vec![], &[])],
        Default::default(),
    );
    let c = p.context.coefficient_context().clone();
    let mut first = rule(17);
    first.exceptions.push(vec![poly(
        &c,
        c.add(
            &c.mul(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
            &c.one(),
        )
        .unwrap(),
    )]);
    first.rhs.push(PreparedTerm {
        shift: [0; 3],
        coefficient: c.zero(),
        denominator: costly(&c, false),
    });
    let owner = Arc::get_mut(
        Arc::get_mut(&mut p)
            .unwrap()
            .owners
            .get_mut(&FINITE_OWNER)
            .unwrap(),
    )
    .unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules = vec![first, rule(29)];
    let mut pieces = Vec::new();
    let stats = p
        .visit_owner_domain_matches(
            FINITE_OWNER,
            &[4, 1, 1],
            &[None, None, Some(1)],
            Some(10),
            limits(5),
            &AtomicBool::new(false),
            |piece| {
                pieces.push(piece);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
    assert_eq!((stats.refinement_cells, stats.refinement_steps), (5, 1));
    assert_eq!(pieces.len(), 5);
    assert!(pieces.iter().all(|piece| piece.disposition()
        == OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 17 }));
    let mut count = 0;
    for a in 4..=8 {
        for b in 1..=9 - a {
            let point = [a, b, 1];
            let containing: Vec<_> = pieces
                .iter()
                .filter(|piece| {
                    (0..3).all(|axis| {
                        point[axis] >= piece.lower()[axis]
                            && piece.upper()[axis].is_none_or(|hi| point[axis] <= hi)
                    }) && point.iter().sum::<u64>()
                        <= u64::from(piece.max_numerator_rank().unwrap())
                })
                .collect();
            assert_eq!(containing.len(), 1);
            assert_eq!(containing[0].max_numerator_rank(), Some(10));
            count += 1;
        }
    }
    assert_eq!(count, 15);
    let mut explicit = Vec::new();
    let mut explicit_predicates = 0;
    for a in 4..=8 {
        let s = p
            .visit_owner_domain_matches(
                FINITE_OWNER,
                &[a, 1, 1],
                &[Some(a), None, Some(1)],
                Some(10),
                limits(0),
                &AtomicBool::new(false),
                |piece| {
                    explicit.push(piece);
                    ControlFlow::Continue(())
                },
            )
            .unwrap();
        explicit_predicates += s.predicates;
    }
    assert_eq!(normalized(&pieces), normalized(&explicit));
    assert_eq!(stats.predicates, explicit_predicates + 2);
}

#[test]
fn rejection_refinement_no_full_split_preserves_original_unknown_and_geometry() {
    let p = retry_fixture(false);
    let (baseline, _) = refined(&p, [0; 3], [None; 3], Some(2), limits(0));
    for (rank, mut policy) in [
        (Some(2), limits(0)),
        (Some(2), limits(2)),
        (None, limits(100)),
        (Some(2), limits(3)),
        (Some(2), limits(3)),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (rank, mut policy))| {
        if i == 3 {
            policy.max_cells = baseline.cells + 2;
        }
        if i == 4 {
            policy.max_coordinate_cells = baseline.coordinate_cells + 2 * 3;
        }
        (rank, policy)
    }) {
        // Keep the same native allowance for every non-admissible split.
        policy.guard_algebra.max_gcd_factor_work = 1000;
        let (stats, pieces) = refined(&p, [0; 3], [None; 3], rank, policy);
        assert_eq!(pieces.len(), 1);
        assert_eq!(
            pieces[0].disposition(),
            OwnerDomainMatchDisposition::Unresolved {
                predicate: equality(0),
            }
        );
        assert_eq!(pieces[0].lower(), &[0; 3]);
        assert_eq!(pieces[0].upper(), &[None; 3]);
        assert_eq!(pieces[0].max_numerator_rank(), rank);
        assert_eq!(stats.refinement_cells, 0);
        assert_eq!(stats.predicates, 2);
        assert_eq!(stats.pieces, 1);
    }
}

#[test]
fn rejection_refinement_global_input_and_same_and_errors_remain_strict() {
    let mut p = retry_fixture(false);
    let mut policy = limits(3);
    policy.max_predicates = 1;
    let error = refuse(&p, Some(2), policy);
    assert!(matches!(
        error.failure,
        OwnerDomainMatchFailure::ResourceLimit {
            resource: "predicates",
            ..
        }
    ));
    assert_eq!(error.predicate, Some(later()));
    assert_eq!(error.stats.refinement_cells, 0);

    // The SAME preflight category stays fatal in mandatory dispatch. With an
    // identically zero required equality, this original denominator is no
    // longer a speculative probe, and its positive-only support cannot refine.
    let mut mandatory = retry_fixture(false);
    let c = mandatory.context.coefficient_context().clone();
    batch(&mut mandatory).rules[0].equalities = vec![poly(&c, c.zero())];
    let error = refuse(&mandatory, Some(2), limits(3));
    assert_eq!(error.failure, failure());
    assert_eq!(error.predicate, Some(later()));
    assert_eq!(error.stats.predicates, 2);
    assert_eq!(error.stats.refinement_cells, 0);
    let mut policy = limits(3);
    policy.guard_algebra.max_input_terms = 2; // original two-term guard fits; later three-term pole does not
    let error = refuse(&p, Some(2), policy);
    assert_eq!(
        error.failure,
        OwnerDomainMatchFailure::Algebra(crate::algebra::IndexedAlgebraError::ResourceLimit {
            resource: "guard coefficient split input terms",
            requested: 3,
            limit: 2,
        })
    );
    assert_eq!(error.predicate, Some(later()));
    assert_eq!(error.stats.refinement_cells, 0);

    // The older same-AND nonzero-witness lookahead retains its strict policy;
    // this patch changes only speculative candidate-rejection lookahead.
    let first = &mut batch(&mut p).rules[0];
    let original = first.equalities.remove(0);
    let denominator = first.rhs.remove(0).denominator;
    first.exceptions.push(vec![original, denominator]);
    let error = refuse(&p, Some(2), limits(3));
    assert_eq!(error.failure, failure());
    assert_eq!(error.predicate, Some(excluded(0, 1)));
    assert_eq!(error.stats.refinement_cells, 0);
}

#[test]
fn rejection_refinement_optional_refusal_preserves_original_positive_tail_unknown() {
    // Representative of the O5Mrky cell: three varying positive coordinates,
    // no unfixed inactive coordinate, and original Unknown in excluded branch3.
    // This is not an exact saved-polynomial replay or a claim of nonempty gap.
    const POSITIVE_OWNER: [bool; 3] = [true; 3];
    let mut p = programs(
        Arc::new(crate::solver::tests::sunset()),
        Some(10),
        vec![input(POSITIVE_OWNER, Some(10), vec![], &[])],
        Default::default(),
    );
    let c = p.context.coefficient_context().clone();
    let mut first = rule(17);
    first.exceptions = vec![
        vec![positive(&c)],
        vec![positive(&c)],
        vec![positive(&c)],
        vec![diagonal(&c)],
    ];
    let set_first = |p: &mut Arc<CandidateOwnerPrograms<3>>, first| {
        let owner = Arc::get_mut(
            Arc::get_mut(p)
                .unwrap()
                .owners
                .get_mut(&POSITIVE_OWNER)
                .unwrap(),
        )
        .unwrap();
        Arc::get_mut(&mut owner.batches[0]).unwrap().rules = vec![first, rule(29)];
    };
    set_first(&mut p, first);
    let run = |p: &CandidateOwnerPrograms<3>, rank| {
        let mut pieces = Vec::new();
        let stats = p
            .visit_owner_domain_matches(
                POSITIVE_OWNER,
                &[3, 1, 1],
                &[None; 3],
                rank,
                limits(100),
                &AtomicBool::new(false),
                |piece| {
                    pieces.push(piece);
                    ControlFlow::Continue(())
                },
            )
            .unwrap();
        (stats, pieces)
    };
    let baseline = [run(&p, Some(10)), run(&p, None)];
    let owner = Arc::get_mut(
        Arc::get_mut(&mut p)
            .unwrap()
            .owners
            .get_mut(&POSITIVE_OWNER)
            .unwrap(),
    )
    .unwrap();
    Arc::get_mut(&mut owner.batches[0]).unwrap().rules[0]
        .rhs
        .push(PreparedTerm {
            shift: [0; 3],
            coefficient: c.zero(),
            denominator: costly(&c, false),
        });
    for (index, rank) in [Some(10), None].into_iter().enumerate() {
        let (stats, pieces) = run(&p, rank);
        assert_eq!(normalized(&pieces), normalized(&baseline[index].1));
        assert_eq!(pieces.len(), 1);
        assert_eq!(
            pieces[0].disposition(),
            OwnerDomainMatchDisposition::Unresolved {
                predicate: excluded(3, 0),
            }
        );
        assert_eq!(pieces[0].lower(), &[3, 1, 1]);
        assert_eq!(pieces[0].upper(), &[None; 3]);
        assert_eq!(pieces[0].max_numerator_rank(), rank);
        assert_eq!(stats.predicates, baseline[index].0.predicates + 1);
        assert_eq!((stats.refinement_steps, stats.refinement_cells), (0, 0));
    }
}

#[test]
fn rejection_refinement_retries_original_after_full_admission_and_keeps_cancel_prefix() {
    let p = retry_fixture(false);
    let mut policy = limits(3);
    policy.max_predicates = 2;
    let error = refuse(&p, Some(2), policy);
    assert_eq!(
        error.failure,
        OwnerDomainMatchFailure::ResourceLimit {
            resource: "predicates",
            requested: 3,
            limit: 2
        }
    );
    assert_eq!(error.predicate, Some(equality(0))); // retry is original, not later pole
    assert_eq!(
        (
            error.stats.predicates,
            error.stats.refinement_cells,
            error.stats.pieces
        ),
        (2, 3, 0)
    );
    let cancellation = AtomicBool::new(false);
    let mut emitted = 0;
    let error = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[None; 3],
            Some(2),
            limits(3),
            &cancellation,
            |_| {
                emitted += 1;
                cancellation.store(true, Ordering::Release);
                ControlFlow::Continue(())
            },
        )
        .unwrap_err();
    assert_eq!(error.failure, OwnerDomainMatchFailure::Cancelled);
    assert_eq!(
        (emitted, error.stats.pieces, error.stats.refinement_cells),
        (1, 1, 3)
    );
}
