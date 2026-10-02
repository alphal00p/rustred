//! Saved alternatives replace only complete ordinary-dispatch pieces.
use super::*;
use crate::solver::RuleDispatchPolicy;

fn alternative(ordinal: usize) -> PreparedRule<3> {
    let mut r = rule(ordinal);
    r.dispatch_policy = RuleDispatchPolicy::AfterBaselinePartitionWholePiece;
    r
}

fn install(p: &mut Arc<CandidateOwnerPrograms<3>>, rules: Vec<PreparedRule<3>>) {
    let b = batch(p);
    let terminals = std::mem::take(&mut b.terminals);
    *b = PreparedOwnerBatch::new(rules, terminals, None);
}

fn geometry(pieces: &[OwnerDomainMatchPiece<3>]) -> Vec<(Vec<u64>, Vec<Option<u64>>, Option<u32>)> {
    pieces
        .iter()
        .map(|p| {
            (
                p.lower().to_vec(),
                p.upper().to_vec(),
                p.max_numerator_rank(),
            )
        })
        .collect()
}

#[test]
fn after_baseline_preserves_partition_and_can_replace_a_piece_of_an_ineligible_input() {
    let mut p = fixture();
    let mut first = rule(11);
    first.fixed[0] = Some(2);
    install(&mut p, vec![first, rule(12)]);
    assert!(
        p.owners[&OWNER].batches[0]
            .whole_piece_alternatives
            .is_empty()
    );
    let (old_stats, old) = refined(
        &p,
        [0; 3],
        [Some(3), Some(0), Some(0)],
        Some(0),
        Default::default(),
    );
    let mut shortcut = alternative(7);
    shortcut.fixed[0] = Some(2);
    let b = batch(&mut p);
    b.rules.insert(0, shortcut);
    b.whole_piece_alternatives = vec![0];
    let (new_stats, new) = refined(
        &p,
        [0; 3],
        [Some(3), Some(0), Some(0)],
        Some(0),
        Default::default(),
    );
    assert_eq!(geometry(&new), geometry(&old));
    assert_eq!(new_stats.split_operations, old_stats.split_operations);
    assert_eq!(new_stats.refinement_cells, old_stats.refinement_cells);
    assert_eq!(
        at(&new, [1, 0, 0]),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 7 }
    );
    for x in [0, 2, 3] {
        assert_eq!(at(&new, [x, 0, 0]), at(&old, [x, 0, 0]));
    }
}

#[test]
fn optional_fixed_and_guard_boundaries_never_split_or_refine_baseline_piece() {
    for kind in 0..3 {
        let mut p = fixture();
        let c = p.context.coefficient_context().clone();
        let mut shortcut = alternative(7);
        match kind {
            0 => shortcut.fixed[0] = Some(2),
            1 => shortcut.equalities.push(minus(&c, 0, 2)),
            _ => shortcut.rhs.push(PreparedTerm {
                shift: [0; 3],
                coefficient: c.zero(),
                denominator: minus(&c, 0, 2),
            }),
        }
        install(&mut p, vec![shortcut, rule(11)]);
        let limits = OwnerDomainMatchLimits {
            max_bounded_refinement_cells: 100,
            refinement_axes: OwnerDomainRefinementAxes::FiniteAxes,
            ..Default::default()
        };
        let (stats, pieces) = refined(&p, [0; 3], [Some(3), Some(0), Some(0)], Some(0), limits);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].upper(), &[Some(3), Some(0), Some(0)]);
        assert_eq!(
            pieces[0].disposition(),
            OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 11 }
        );
        assert_eq!(
            (
                stats.split_operations,
                stats.refinement_steps,
                stats.refinement_cells
            ),
            (0, 0, 0)
        );
    }
}

#[test]
fn optional_excluded_conjunction_can_use_a_later_nonzero_atom_but_keeps_original_poles() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut shortcut = alternative(7);
    shortcut.exceptions.push(vec![
        poly(
            &c,
            c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
        ),
        poly(
            &c,
            c.add(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
        ),
    ]);
    install(&mut p, vec![shortcut, rule(11)]);
    let (stats, pieces) = refined(
        &p,
        [0; 3],
        [None, None, Some(0)],
        Some(0),
        Default::default(),
    );
    assert_eq!(pieces.len(), 1);
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 7 }
    );
    assert_eq!((stats.split_operations, stats.refinement_steps), (0, 0));
    // Even a zero coefficient keeps its original denominator's forbidden face.
    batch(&mut p).rules[0].rhs.push(PreparedTerm {
        shift: [0; 3],
        coefficient: c.zero(),
        denominator: minus(&c, 0, 2),
    });
    let pieces = collect(&p, &[0; 3], &[None, None, Some(0)], Some(0));
    assert_eq!(pieces.len(), 1);
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 11 }
    );
    let pieces = collect(&p, &[2, 0, 0], &[None, None, Some(0)], Some(0));
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 7 }
    );
}

#[test]
fn alternatives_never_fill_gaps_override_terminals_source_failures_or_unresolved_rules() {
    let mut p = fixture();
    install(&mut p, vec![alternative(7)]);
    assert_eq!(
        collect(&p, &[0; 3], &[Some(0); 3], Some(0))[0].disposition(),
        OwnerDomainMatchDisposition::ExactGap
    );
    batch(&mut p)
        .terminals
        .insert(IntegralKey::try_new([1, 1, 0]).unwrap());
    assert_eq!(
        collect(&p, &[0; 3], &[Some(0); 3], Some(0))[0].disposition(),
        OwnerDomainMatchDisposition::Terminal { batch: 0 }
    );
    let c = p.context.coefficient_context().clone();
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .source_conditions = vec![minus(&c, 0, 1)];
    assert_eq!(
        collect(&p, &[0; 3], &[Some(0); 3], Some(0))[0].disposition(),
        OwnerDomainMatchDisposition::InvalidSourceCondition { ordinal: 0 }
    );
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .source_conditions
        .clear();
    batch(&mut p).terminals.clear();
    let mut baseline = rule(11);
    baseline.equalities.push(poly(
        &c,
        c.sub(&c.index(0).unwrap(), &c.index(1).unwrap()).unwrap(),
    ));
    install(&mut p, vec![alternative(7), baseline]);
    let pieces = collect(&p, &[0; 3], &[None, None, Some(0)], Some(0));
    assert!(matches!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::Unresolved {
            predicate: OwnerDomainPredicate::Equality { rule: 11, .. }
        }
    ));
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .zero_sectors
        .insert(OWNER);
    assert_eq!(
        collect(&p, &[0; 3], &[Some(0); 3], Some(0))[0].disposition(),
        OwnerDomainMatchDisposition::ExactZeroSector
    );
}

#[test]
fn optional_selection_is_confined_to_the_selected_immutable_batch() {
    let mut p = fixture();
    install(&mut p, vec![rule(11)]);
    let owner = Arc::get_mut(
        Arc::get_mut(&mut p)
            .unwrap()
            .owners
            .get_mut(&OWNER)
            .unwrap(),
    )
    .unwrap();
    owner.batches.push(Arc::new(PreparedOwnerBatch::new(
        vec![alternative(7), rule(21)],
        Default::default(),
        None,
    )));
    let pieces = collect(&p, &[0; 3], &[Some(0); 3], Some(0));
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 11 }
    );
    // A marked rule in the first batch does not borrow a later batch's baseline.
    install(&mut p, vec![alternative(8)]);
    let pieces = collect(&p, &[0; 3], &[Some(0); 3], Some(0));
    assert_eq!(
        pieces[0].disposition(),
        OwnerDomainMatchDisposition::SelectedRule { batch: 1, rule: 7 }
    );
}

#[test]
fn optional_guard_budget_failure_and_cancellation_remain_typed_not_fallback_success() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    let mut shortcut = alternative(7);
    shortcut.equalities.push(poly(&c, c.zero()));
    install(&mut p, vec![shortcut, rule(11)]);
    let mut emitted = 0;
    let error = p
        .visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[Some(0); 3],
            Some(0),
            OwnerDomainMatchLimits {
                max_predicates: 0,
                ..Default::default()
            },
            &AtomicBool::new(false),
            |_| {
                emitted += 1;
                ControlFlow::Continue(())
            },
        )
        .unwrap_err();
    assert_eq!(emitted, 0);
    assert!(format!("{error:?}").contains("predicates"));
    assert!(
        p.visit_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[Some(0); 3],
            Some(0),
            Default::default(),
            &AtomicBool::new(true),
            |_| ControlFlow::Continue(())
        )
        .is_err()
    );
}

#[test]
fn concrete_evaluator_needs_baseline_and_selected_alternative_still_has_descent_gate() {
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .zero_sectors
        .clear();
    let mut shortcut = alternative(7);
    shortcut.rhs.push(PreparedTerm {
        shift: [1, 0, 0],
        coefficient: c.one(),
        denominator: poly(&c, c.one()),
    });
    install(&mut p, vec![shortcut]);
    let apply = |p: &CandidateOwnerPrograms<3>| {
        let shared = &p.context.shared;
        let owner = &p.owners[&OWNER];
        let b = &owner.batches[0];
        CandidateEvaluator {
            context: &shared.context,
            root_sector: owner.root,
            ordering: &owner.ordering,
            rules: &b.rules,
            whole_piece_alternatives: &b.whole_piece_alternatives,
            source_conditions: &shared.source_conditions,
            zero_sectors: &shared.zero_sectors,
            limits: p.context.limits,
        }
        .apply(
            &IntegralKey::try_new([2, 1, 0]).unwrap(),
            &mut ReductionRequest::default(),
            &mut ReductionStatistics::default(),
        )
    };
    assert!(matches!(
        apply(&p),
        Err(CandidateReductionError::Uncovered { .. })
    ));
    batch(&mut p).rules.push(rule(11));
    assert!(matches!(
        apply(&p),
        Err(CandidateReductionError::NonDescending { rule: 7, .. })
    ));
    batch(&mut p).rules[0].rhs[0].shift = [-1, 0, 0];
    let result = apply(&p).unwrap();
    assert_eq!(
        result.keys().collect::<Vec<_>>(),
        vec![&IntegralKey::try_new([1, 1, 0]).unwrap()]
    );
    batch(&mut p).rules[0].rhs[0].denominator = minus(&c, 0, 2);
    assert!(
        apply(&p).unwrap().is_empty(),
        "pole falls back to the unchanged empty baseline RHS"
    );
}

#[test]
fn after_baseline_keeps_exact_power_predicates_and_effective_rank() {
    use crate::solver::candidate_reduction::power_domain::DomainPowerBounds;
    let mut p = fixture();
    install(&mut p, vec![rule(11)]);
    let bounds = DomainPowerBounds {
        max_positive_power: Some(5),
        min_power_difference: Some(3),
        max_power_difference: Some(3),
    };
    let capture = |p: &CandidateOwnerPrograms<3>| {
        let mut pieces = Vec::new();
        p.visit_power_bounded_owner_domain_matches(
            OWNER,
            &[0; 3],
            &[Some(3), Some(3), Some(2)],
            Some(2),
            bounds,
            Default::default(),
            &AtomicBool::new(false),
            |piece| {
                pieces.push(piece);
                ControlFlow::Continue(())
            },
        )
        .unwrap();
        pieces
    };
    let old = capture(&p);
    install(&mut p, vec![alternative(7), rule(11)]);
    let new = capture(&p);
    assert_eq!(geometry(&old), geometry(&new));
    assert!(!new.is_empty());
    for (old, new) in old.iter().zip(&new) {
        assert_eq!(old.power_bounds(), new.power_bounds());
        assert_eq!(new.power_bounds(), bounds);
        assert_eq!(
            new.disposition(),
            OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 7 }
        );
    }
}

#[test]
fn native_applied_pipeline_uses_marked_rule_and_reports_bad_descent_without_fallback() {
    use crate::solver::candidate_reduction::owners::domains::{
        OwnerAppliedEvent, OwnerAppliedProblemKind,
    };
    let mut p = fixture();
    let c = p.context.coefficient_context().clone();
    Arc::get_mut(&mut Arc::get_mut(&mut p).unwrap().context)
        .unwrap()
        .shared
        .zero_sectors
        .clear();
    let mut shortcut = alternative(7);
    shortcut.rhs.push(PreparedTerm {
        shift: [-1, 0, 0],
        coefficient: c.one(),
        denominator: poly(&c, c.one()),
    });
    install(&mut p, vec![shortcut, rule(11)]);
    for raising in [false, true] {
        batch(&mut p).rules[0].rhs[0].shift[0] = if raising { 1 } else { -1 };
        let mut chosen = Vec::new();
        let mut edges = Vec::new();
        let mut problems = Vec::new();
        let mut finished = Vec::new();
        p.visit_owner_applied_successors(
            OWNER,
            &[1, 0, 0],
            &[Some(1), Some(0), Some(0)],
            Some(0),
            Default::default(),
            &AtomicBool::new(false),
            |event| {
                match event {
                    OwnerAppliedEvent::Classified(piece) => chosen.push(piece.disposition()),
                    OwnerAppliedEvent::Successor(edge) => {
                        edges.push((*edge.shift, edge.target_lower.to_vec()))
                    }
                    OwnerAppliedEvent::Problem(problem) => problems.push(problem.kind),
                    OwnerAppliedEvent::RuleFinished {
                        source,
                        successors,
                        problems,
                    } => finished.push((source.disposition(), successors, problems)),
                    OwnerAppliedEvent::OptionalCoefficientRefusal { .. } => {
                        panic!("constant fixture has no optional refusal")
                    }
                }
                ControlFlow::Continue(())
            },
        )
        .unwrap();
        let selected = OwnerDomainMatchDisposition::SelectedRule { batch: 0, rule: 7 };
        assert_eq!(chosen, vec![selected]);
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].0, selected);
        if raising {
            assert!(edges.is_empty());
            assert!(matches!(
                problems.as_slice(),
                [OwnerAppliedProblemKind::DescentNotEstablished { .. }]
            ));
            assert_eq!(finished[0].2, 1);
        } else {
            assert!(problems.is_empty());
            assert_eq!(edges, vec![([-1, 0, 0], vec![0, 0, 0])]);
            assert_eq!(finished[0].1, 1);
        }
    }
}
