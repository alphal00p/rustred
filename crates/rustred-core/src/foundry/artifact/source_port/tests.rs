use std::sync::Arc;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::solver::{
    CoordinateCase, ExceptionalConditions, Integral, RuleCandidate, SectorConfig, SectorRule,
    SectorSolution, SectorSolveOptions, SectorSolver, SourceSystem, Term,
};

use super::{
    AffineApplicationDomain, AffineOwnershipRole, SourcePortAudit, SourcePortAuditError, geometry,
};

#[test]
fn message_location_keeps_typed_proof_errors_intact() {
    assert_eq!(
        SourcePortAuditError::message("guard limit")
            .with_message_context(|| "lowering rule 7".into())
            .to_string(),
        "lowering rule 7: guard limit",
    );
    let typed = SourcePortAuditError::ResourceBudgetExhausted { resource: "atoms" }
        .with_message_context(|| panic!("typed errors must retain their structure"));
    assert!(matches!(
        typed,
        SourcePortAuditError::ResourceBudgetExhausted { resource: "atoms" }
    ));
}

#[test]
fn lowering_storage_failure_identifies_sector_and_retained_rule() {
    let (audit, solution) = solved_tadpole();
    let mut limits = super::SourcePortLimits::default();
    limits.rule_derivation.max_domain_bound_endpoint_cells = 1;
    let failure = audit
        .with_limits(limits)
        .install_complete(tadpole(), [([true], None, solution)])
        .unwrap_err();
    let message = failure.to_string();
    assert!(
        message.contains("lowering sector [true], retained rule 0/1"),
        "{message}"
    );
    assert!(message.contains("domain bound endpoint cells"), "{message}");
}

#[test]
fn affine_ownership_diagnostic_preserves_sector_and_exact_constraints() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let equation = context.coefficient_fixture("n0 - n1").numerator;
    let domain = AffineApplicationDomain::try_new(
        vec![true, false],
        vec![None, Some(2)],
        vec![equation.clone()],
    )
    .unwrap();
    let error = SourcePortAuditError::UnsupportedAffineOwnership {
        domain,
        role: AffineOwnershipRole::Exceptional,
    };
    let (sector, fixed, equations, role) = error.affine_ownership().expect("typed affine error");
    assert_eq!(sector, &[true, false]);
    assert_eq!(fixed, &[None, Some(2)]);
    assert_eq!(equations, &[equation]);
    assert_eq!(role, AffineOwnershipRole::Exceptional);
    assert!(error.to_string().contains("sector=[true, false]"));
}

pub(super) fn tadpole() -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    IntegralFamily::new(
        "source-port-certificate-tadpole",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

pub(super) fn solved_tadpole() -> (SourcePortAudit<1>, SectorSolution<1>) {
    let family = tadpole();
    let zeros: Arc<[[bool; 1]]> = Arc::from([[false]]);
    let source = SourceSystem::from_family(&family).unwrap();
    let solver = SectorSolver::new(
        &source,
        [true],
        SectorConfig {
            zero_sectors: zeros.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    let solution = solver.solve_sector(SectorSolveOptions::default()).unwrap();
    (SourcePortAudit::try_new(&family, zeros).unwrap(), solution)
}

#[test]
fn ordinary_replay_and_unbounded_cover_close_the_generated_tadpole_report() {
    let (audit, solution) = solved_tadpole();
    assert_eq!(audit.original_source_count(), 1);
    assert_eq!(audit.proved_zero_sector_count(), 1);
    let report = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(report.rules, 1);
    assert_eq!(report.exact_replayed_rules, 1);
    assert_eq!(report.uniformly_descending_rules, 1);
    assert_eq!(report.additional_replay_guard_branches, 0);
    assert_eq!(report.finite_terminals, 1);
    assert_eq!(report.checked_rule_uncovered_boxes, 0);
    assert_eq!(report.stored_guard_uncovered_boxes, 0);
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    audit.validate_sector_census(&[report]).unwrap();
}

#[test]
fn total_excess_audit_labels_its_scope_and_preserves_full_identity_checks() {
    let (audit, solution) = solved_tadpole();
    let bounded = audit
        .audit_sector_through_total_excess([true], None, &solution, 30)
        .unwrap();
    assert_eq!(bounded.max_total_excess_degree, Some(30));
    assert_eq!(bounded.exact_replayed_rules, solution.rules.len());
    assert_eq!(bounded.uniformly_descending_rules, solution.rules.len());
    assert_eq!(bounded.checked_rule_uncovered_boxes, 0);
    assert!(bounded.issues.is_empty(), "{:?}", bounded.issues);
    assert_eq!(
        audit
            .audit_sector([true], None, &solution)
            .unwrap()
            .max_total_excess_degree,
        None
    );

    // Even degree zero (the finite terminal alone) does not turn an invalid
    // candidate identity into a replayed rule or skip the full identity gate.
    let (audit, mut corrupted) = solved_tadpole();
    corrupted.rules[0].candidate.rhs[0].coefficient =
        -corrupted.rules[0].candidate.rhs[0].coefficient.clone();
    let report = audit
        .audit_sector_through_total_excess([true], None, &corrupted, 0)
        .unwrap();
    assert_eq!(report.max_total_excess_degree, Some(0));
    assert_eq!(report.exact_replayed_rules, 0);
    assert!(!report.issues.is_empty());
}

#[test]
fn bounded_terminal_cover_is_not_unbounded_closure() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules.clear();
    let bounded = audit
        .audit_sector_through_total_excess([true], None, &solution, 0)
        .unwrap();
    assert!(bounded.issues.is_empty());
    assert_eq!(bounded.max_total_excess_degree, Some(0));
    let whole = audit.audit_sector([true], None, &solution).unwrap();
    assert!(whole.checked_rule_unbounded_boxes > 0);
    assert!(!whole.issues.is_empty());
    assert!(
        audit
            .install_complete(tadpole(), [([true], None, solution)])
            .is_err()
    );
}

#[test]
fn affine_candidates_are_omitted_only_after_an_independent_complete_cover() {
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let zeros: Arc<[[bool; 3]]> = Arc::from([
        [false, false, false],
        [true, false, false],
        [false, true, false],
        [false, false, true],
    ]);
    let sources = SourceSystem::<3>::from_family(&family).unwrap();
    let audit = SourcePortAudit::try_new(&family, zeros.clone()).unwrap();
    let solver = SectorSolver::new(
        &sources,
        [true; 3],
        SectorConfig {
            zero_sectors: zeros.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut solution = solver.solve_sector(SectorSolveOptions::default()).unwrap();
    let proved_rules = solution.rules.len();
    let template = &sources.rows()[0][0].coefficient;
    let first = template
        .variable(&template.variables()[sources.index_variables()[0]])
        .unwrap();
    let second = template
        .variable(&template.variables()[sources.index_variables()[1]])
        .unwrap();
    let crate::solver::AffineIntersection::Affine(affine) =
        crate::solver::AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &[&first - &second],
            sources.index_variables(),
            &[true; 3],
        )
        .unwrap()
    else {
        panic!("equal-index test case must remain affine");
    };
    let case = crate::solver::Case::from(affine);
    // This deliberately has no source certificate: it may only be discarded,
    // never admitted as a rule. The complete independent coordinate cover is
    // the authority for all points, including the affine stratum.
    solution.rules.push(SectorRule {
        candidate: RuleCandidate {
            target: case.integral(),
            case,
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: Default::default(),
    });
    let coordinate = CoordinateCase::generic();
    solution.rules.push(SectorRule {
        candidate: RuleCandidate {
            target: coordinate.integral(),
            case: coordinate.into(),
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions {
            branches: vec![vec![first - second]],
            ..Default::default()
        },
    });
    let complete = audit.audit_sector([true; 3], None, &solution).unwrap();
    // The affine target now reaches the replay stage through its coordinate
    // face prefilter, but the deliberately empty synthetic candidate has no
    // certificate and therefore is not retained as a rule.
    assert_eq!(complete.rules, proved_rules + 2);
    assert_eq!(complete.exact_replayed_rules, proved_rules);
    assert_eq!(complete.uniformly_descending_rules, proved_rules);
    assert_eq!(complete.redundant_affine_rules, 2);
    assert_eq!(complete.checked_rule_uncovered_boxes, 0);
    assert!(complete.issues.is_empty(), "{:?}", complete.issues);

    let terminals = std::mem::take(&mut solution.finite_residuals);
    let missing_point = audit.audit_sector([true; 3], None, &solution).unwrap();
    assert_eq!(missing_point.redundant_affine_rules, 0);
    assert!(missing_point.checked_rule_uncovered_boxes > 0);
    assert_eq!(missing_point.checked_rule_unbounded_boxes, 0);
    solution.finite_residuals = terminals;

    let proved = solution.rules.drain(..proved_rules).collect::<Vec<_>>();
    let incomplete = audit.audit_sector([true; 3], None, &solution).unwrap();
    assert_eq!(incomplete.redundant_affine_rules, 0);
    assert!(incomplete.checked_rule_unbounded_boxes > 0);
    assert!(
        incomplete
            .issues
            .iter()
            .any(|issue| issue.contains("cannot be omitted"))
    );
    assert!(
        incomplete
            .issues
            .iter()
            .any(|issue| issue.contains("equations="))
    );
    solution.rules.splice(..0, proved);

    let mut sectors = vec![([true; 3], None, solution)];
    for sector in [
        [false, true, true],
        [true, false, true],
        [true, true, false],
    ] {
        let solver = SectorSolver::new(
            &sources,
            sector,
            SectorConfig {
                zero_sectors: zeros.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        sectors.push((
            sector,
            None,
            solver.solve_sector(Default::default()).unwrap(),
        ));
    }
    let artifact = audit.install_complete(family, sectors).unwrap();
    let bytes = artifact.encode_durable().unwrap();
    let loaded = crate::foundry::artifact::ClosedArtifact::decode_durable(&bytes).unwrap();
    assert_eq!(loaded.encode_durable().unwrap(), bytes);
    let mut direct = crate::reduction::Reducer::new(&artifact).unwrap();
    let mut cold = crate::reduction::Reducer::new(&loaded).unwrap();
    for powers in [[1, 1, 1], [2, 2, 1], [3, 3, 2]] {
        let target = crate::family::IntegralKey::try_new(powers).unwrap();
        assert_eq!(
            direct.reduce_unit_mass(&target).unwrap().terms(),
            cold.reduce_unit_mass(&target).unwrap().terms(),
        );
    }
}

#[test]
fn removing_terminal_and_removing_rule_distinguish_finite_and_infinite_gaps() {
    let (audit, mut solution) = solved_tadpole();
    let terminals = std::mem::take(&mut solution.finite_residuals);
    let finite = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(finite.checked_rule_uncovered_boxes, 1);
    assert_eq!(finite.checked_rule_unbounded_boxes, 0);
    solution.finite_residuals = terminals;
    solution.rules.clear();
    let infinite = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(infinite.checked_rule_uncovered_boxes, 1);
    assert_eq!(infinite.checked_rule_unbounded_boxes, 1);
}

#[test]
fn mutated_rhs_cannot_pass_regenerated_source_replay() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].candidate.rhs[0].coefficient =
        -solution.rules[0].candidate.rhs[0].coefficient.clone();
    let report = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(report.exact_replayed_rules, 0);
    assert!(!report.issues.is_empty());
    assert_eq!(report.checked_rule_unbounded_boxes, 1);
}

#[test]
fn invalid_provenance_ordinal_is_reported_without_trusting_stored_equations() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].candidate.sources[0].basis_row = usize::MAX;
    let report = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(report.exact_replayed_rules, 0);
    assert!(report.issues[0].contains("ordinal"));
}

#[test]
fn omitted_serialized_pole_is_recomputed_and_retained_as_an_extra_obligation() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].exceptions = ExceptionalConditions::default();
    let report = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(report.exact_replayed_rules, 1);
    assert_eq!(report.additional_replay_guard_branches, 1);
    assert_eq!(report.checked_rule_uncovered_boxes, 0);
    assert_eq!(report.finite_terminals, 1);
}

#[test]
fn malformed_seed_and_consistently_mutated_rhs_do_not_forge_ordinary_provenance() {
    let (audit, mut solution) = solved_tadpole();
    let solver = SectorSolver::new(
        &audit.sources,
        [true],
        SectorConfig {
            zero_sectors: audit.zero_sectors.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    let order = crate::solver::IntegralOrder::new([true], [false]);
    let mut source = solution.rules[0].candidate.sources[0];
    source.seed.shifts[0] += 1;
    assert_ne!(source.seed.integral[0].value(), source.seed.shifts[0]);
    // Fabricate the matching RHS with the same malformed transport. Merely
    // replaying this transport twice would reproduce an invalid IBP exactly.
    let row = crate::solver::instantiate_source_port(
        &solver.basis()[source.basis_row],
        &source.seed,
        audit.sources.index_variables(),
        audit.sources.fixed(),
        &order,
        &audit.zero_sectors,
        None,
    )
    .unwrap();
    let (target, rhs) =
        crate::solver::canonicalize_source_port(row, audit.sources.index_variables()).unwrap();
    assert_eq!(target, solution.rules[0].candidate.target);
    assert_ne!(rhs, solution.rules[0].candidate.rhs);
    solution.rules[0].candidate.rhs = rhs;
    solution.rules[0].candidate.sources = vec![source];
    solution.rules[0].exceptions = crate::solver::extract_exceptions(
        &solution.rules[0].candidate,
        audit.sources.index_variables(),
        &[true],
    )
    .unwrap();
    let report = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(report.exact_replayed_rules, 0);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.contains("seed coefficient translation"))
    );
    assert_eq!(report.checked_rule_unbounded_boxes, 1);
}

#[test]
fn numeric_seed_shifts_and_wrong_seed_patterns_are_rejected() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].candidate.sources[0].seed.integral = Integral::numeric([2]).unwrap();
    solution.rules[0].candidate.sources[0].seed.shifts = [0];
    let wrong_pattern = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(wrong_pattern.exact_replayed_rules, 0);
    assert!(
        wrong_pattern
            .issues
            .iter()
            .any(|issue| issue.contains("seed symbolic pattern"))
    );

    // Numeric seed coordinates are replacement values. Their shifts must be
    // zero; the underlying instantiator intentionally ignores this field.
    let fixed = CoordinateCase::new([Some(2)]).unwrap();
    solution.rules[0].candidate.case = fixed.into();
    solution.rules[0].candidate.target = fixed.integral();
    solution.rules[0].candidate.sources[0].seed.shifts = [1];
    let numeric_shift = audit.audit_sector([true], None, &solution).unwrap();
    assert_eq!(numeric_shift.exact_replayed_rules, 0);
    assert!(
        numeric_shift
            .issues
            .iter()
            .any(|issue| issue.contains("seed coefficient translation"))
    );
}

#[test]
fn false_and_duplicate_zero_censuses_are_rejected() {
    assert!(SourcePortAudit::<1>::try_new(&tadpole(), Arc::from([[true]])).is_err());
    assert!(SourcePortAudit::<1>::try_new(&tadpole(), Arc::from([[false], [false]])).is_err());
    let (audit, solution) = solved_tadpole();
    assert!(audit.validate_sector_census(&[]).is_err());
    assert!(audit.audit_sector([false], None, &solution).is_err());
}

#[test]
fn retained_replay_keeps_original_ids_offsets_and_nonzero_weights() {
    let (audit, solution) = solved_tadpole();
    let solver = SectorSolver::new(
        &audit.sources,
        [true],
        SectorConfig {
            zero_sectors: audit.zero_sectors.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    let rule = &solution.rules[0];
    let boxes =
        geometry::application_boxes(rule, audit.sources.index_variables(), &[true], &[]).unwrap();
    let replay = super::replay::replay_rule(
        &audit.sources,
        &audit.original_row_ids,
        &audit.original_sources,
        solver.basis(),
        solver.ordering(),
        &audit.zero_sectors,
        rule,
        &boxes,
        None,
    )
    .unwrap();
    assert_eq!(
        replay.ordinary.normalization,
        super::certificate::OriginalRowNormalization::OriginalGeneratorOrdinaryV1
    );
    assert_eq!(replay.ordinary.contributions.len(), 1);
    for contribution in &replay.ordinary.contributions {
        assert_eq!(
            contribution.source_row,
            crate::identity::RowId::OrdinaryIbp {
                contraction_momentum: 0,
                differentiated_loop: 0,
            }
        );
        assert_eq!(contribution.offset, [-1]);
        assert!(!contribution.weight.is_zero());
    }
}

#[test]
fn retained_requests_are_joined_before_zero_weight_filtering() {
    let context = CoefficientContext::new(["d"]);
    let first = crate::identity::RowId::OrdinaryIbp {
        contraction_momentum: 0,
        differentiated_loop: 0,
    };
    let second = crate::identity::RowId::OrdinaryIbp {
        contraction_momentum: 1,
        differentiated_loop: 0,
    };
    let replay = super::certificate::OriginalSourceReplay::retain_checked(
        vec![(first.clone(), [1, 2]), (second.clone(), [3, 4])],
        vec![context.zero(), context.integer(2)],
    )
    .unwrap();
    assert_eq!(replay.contributions.len(), 1);
    assert_eq!(replay.contributions[0].source_row, second);
    assert_eq!(replay.contributions[0].offset, [3, 4]);
    assert_eq!(replay.contributions[0].weight, context.integer(2));
    assert!(
        super::certificate::OriginalSourceReplay::retain_checked(vec![(first, [1, 2])], vec![],)
            .is_err()
    );
}

#[test]
fn target_relative_requests_preserve_numeric_seed_displacements() {
    let (_, mut rule) = boundary_rule("1");
    let case = CoordinateCase::new([None, Some(1)]).unwrap();
    rule.candidate.case = case.into();
    rule.candidate.target = case.integral();
    let seed = crate::solver::Seed {
        integral: Integral::new([
            crate::solver::Power::new(true, 2).unwrap(),
            crate::solver::Power::new(false, 3).unwrap(),
        ]),
        shifts: [2, 0],
    };
    // Translate the original row by [1,2] before fixing target n1=1;
    // its original numeric source argument is then 3, not target value 1.
    assert_eq!(
        super::certificate::source_offset(&rule, &seed, &[-1, 0]),
        [1, 2]
    );
}

fn boundary_rule(coefficient: &str) -> (CoefficientContext, SectorRule<2>) {
    let context = CoefficientContext::new(["n0", "n1", "d"]);
    let case = CoordinateCase::generic();
    let candidate = RuleCandidate {
        case: case.into(),
        target: case.integral(),
        rhs: vec![Term {
            integral: Integral::symbolic([1, -2]).unwrap(),
            coefficient: context.coefficient_fixture(coefficient),
        }],
        sources: Vec::new(),
        stats: Default::default(),
    };
    (
        context,
        SectorRule {
            candidate,
            exceptions: ExceptionalConditions::default(),
        },
    )
}

#[test]
fn affine_exception_partition_keeps_exact_ray_out_of_box_coverage() {
    let (context, mut rule) = boundary_rule("1");
    rule.exceptions.branches = vec![vec![context.coefficient_fixture("1+n0-2*n1").numerator]];
    let partition = geometry::application_partition(&rule, &[0, 1], &[false, false], &[]).unwrap();
    assert_eq!(partition.boxes.len(), 1);
    assert_eq!(partition.boxes[0].lower(), &[0, 0]);
    assert_eq!(partition.boxes[0].upper(), &[None, None]);
    assert_eq!(partition.affine_exclusions.len(), 1);
    let excluded = &partition.affine_exclusions[0];
    assert!(excluded.contains_powers(&[-1, 0]));
    assert!(excluded.contains_powers(&[-3, -1]));
    assert!(!excluded.contains_powers(&[0, 0]));
    // The box-only caller must still refuse to treat this prefilter as a
    // complete owner: its infinite exceptional ray is carried separately.
    assert!(matches!(
        geometry::application_boxes(&rule, &[0, 1], &[false, false], &[]),
        Err(SourcePortAuditError::UnsupportedAffineOwnership {
            role: AffineOwnershipRole::Exceptional,
            ..
        })
    ));
}

#[test]
fn whole_ray_descent_drops_only_exactly_vanishing_activation_boundary_terms() {
    let (_, rule) = boundary_rule("n0");
    let cells = geometry::application_boxes(&rule, &[0, 1], &[false, true], &[]).unwrap();
    geometry::prove_descent(
        &rule,
        &cells,
        &[],
        &[false, true],
        crate::sector::OrderingPolicy::SpiredUncutV1,
        &[0, 1],
    )
    .unwrap();
    let (_, unsafe_rule) = boundary_rule("1");
    assert!(
        geometry::prove_descent(
            &unsafe_rule,
            &cells,
            &[],
            &[false, true],
            crate::sector::OrderingPolicy::SpiredUncutV1,
            &[0, 1]
        )
        .is_err()
    );
}

#[test]
fn affine_descent_excludes_only_proved_impossible_activation_cells() {
    let context = CoefficientContext::new(["n0", "n1"]);
    let sector = [false, false];
    let make_rule = |equation: &str| {
        let crate::solver::AffineIntersection::Affine(affine) =
            crate::solver::AffineCase::from_coordinate(
                &CoordinateCase::generic(),
                &[context.coefficient_fixture(equation).numerator],
                &[0, 1],
                &sector,
            )
            .unwrap()
        else {
            panic!("expected affine ray")
        };
        let case = crate::solver::Case::from(affine);
        SectorRule {
            candidate: RuleCandidate {
                target: case.integral(),
                case,
                rhs: vec![Term {
                    integral: Integral::symbolic([1, 0]).unwrap(),
                    coefficient: context.one(),
                }],
                sources: Vec::new(),
                stats: Default::default(),
            },
            exceptions: ExceptionalConditions::default(),
        }
    };
    let boxes =
        vec![crate::foundry::completion::LatticeBox::try_new([0, 0], [None, None]).unwrap()];
    geometry::prove_descent(
        &make_rule("1+n0-2*n1"),
        &boxes,
        &[],
        &sector,
        crate::sector::OrderingPolicy::SpiredUncutV1,
        &[0, 1],
    )
    .unwrap();
    // n0=n1=0 is now on the locus, so raising n0 creates a genuine higher
    // sector. Reusing the positive test's parity shortcut would be unsound.
    assert!(
        geometry::prove_descent(
            &make_rule("n0-2*n1"),
            &boxes,
            &[],
            &sector,
            crate::sector::OrderingPolicy::SpiredUncutV1,
            &[0, 1]
        )
        .is_err()
    );
}

#[test]
fn affine_target_removes_relative_coordinate_exceptions_from_replay_prefilter() {
    let context = CoefficientContext::new(["n0", "n1", "n2", "n3"]);
    let sector = [false; 4];
    let crate::solver::AffineIntersection::Affine(affine) =
        crate::solver::AffineCase::from_coordinate(
            &CoordinateCase::generic(),
            &[context.coefficient_fixture("1+n0-2*n1").numerator],
            &[0, 1, 2, 3],
            &sector,
        )
        .unwrap()
    else {
        panic!("expected affine target")
    };
    let case = crate::solver::Case::from(affine);
    let mut rule = SectorRule {
        candidate: RuleCandidate {
            target: case.integral(),
            case,
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions {
            branches: vec![vec![context.coefficient_fixture("n2").numerator]],
        },
    };
    let partition = geometry::application_partition(&rule, &[0, 1, 2, 3], &sector, &[]).unwrap();
    assert!(partition.affine_exclusions.is_empty());
    assert_eq!(partition.boxes.len(), 1);
    assert_eq!(partition.boxes[0].lower()[2], 1); // n2 <= -1, not n2=0
    assert_eq!(partition.boxes[0].upper()[2], None);
    // A genuinely new coupled equality cannot be replaced with its fixed
    // face (here the entire box); doing so would erase a valid owner.
    rule.exceptions.branches = vec![vec![context.coefficient_fixture("n2-n3").numerator]];
    let partition = geometry::application_partition(&rule, &[0, 1, 2, 3], &sector, &[]).unwrap();
    assert_eq!(partition.affine_exclusions.len(), 1);
    assert_eq!(partition.boxes[0].lower(), &[0; 4]);
}

#[test]
fn zero_projection_checks_all_physical_sign_cells_not_the_assumed_sector() {
    let (context, mut rule) = boundary_rule("1");
    let case = CoordinateCase::new([None, Some(1)]).unwrap();
    rule.candidate.case = case.into();
    rule.candidate.target = case.integral();
    rule.candidate.rhs.clear();
    let column = Integral::new([
        crate::solver::Power::new(true, 1).unwrap(),
        crate::solver::Power::new(false, 0).unwrap(),
    ]);
    let all = geometry::application_boxes(&rule, &[0, 1], &[false, true], &[]).unwrap();
    assert!(
        !geometry::uniformly_zero_column(&rule, column, &all, &[false, true], &[[false, false]])
            .unwrap()
    );
    rule.exceptions.branches = vec![vec![context.parameter("n0").unwrap().numerator.clone()]];
    let restricted = geometry::application_boxes(&rule, &[0, 1], &[false, true], &[]).unwrap();
    assert!(
        geometry::uniformly_zero_column(
            &rule,
            column,
            &restricted,
            &[false, true],
            &[[false, false]]
        )
        .unwrap()
    );
}

#[test]
fn zero_product_projection_uses_the_original_coefficient_on_activation_faces() {
    let (context, mut rule) = boundary_rule("1");
    let case = CoordinateCase::new([None, Some(1)]).unwrap();
    rule.candidate.case = case.into();
    rule.candidate.target = case.integral();
    rule.candidate.rhs.clear();
    let boxes = geometry::application_boxes(&rule, &[0, 1], &[false, true], &[]).unwrap();
    let mut term = Term {
        integral: Integral::new([
            crate::solver::Power::new(true, 1).unwrap(),
            crate::solver::Power::new(false, 0).unwrap(),
        ]),
        coefficient: context.coefficient_fixture("n0"),
    };
    let check = |term: &Term<2, crate::algebra::Coefficient>| {
        geometry::uniformly_zero_term(
            &rule,
            term,
            &boxes,
            &[false, true],
            &[[false, false]],
            &[0, 1],
        )
        .unwrap()
    };
    // n0 < 0: both columns absent, hence a proved-zero sector.
    // n0 = 0: the first column activates, but its own coefficient is zero.
    assert!(check(&term));
    term.coefficient = context.coefficient_fixture("1");
    assert!(
        !check(&term),
        "a nonzero activation contribution must remain"
    );
    term.coefficient = context.coefficient_fixture("n0+1");
    assert!(
        !check(&term),
        "a different coefficient root is not the boundary"
    );
    term.coefficient = context.coefficient_fixture("n0/(n1-1)");
    assert!(
        !check(&term),
        "0/0 on the fixed face is not a zero coefficient"
    );
}

#[test]
fn all_two_loop_pinches_replay_with_exact_zero_product_boundaries() {
    let context = CoefficientContext::new(["d", "m"]);
    let mass = context.parameter("m").unwrap();
    let family = IntegralFamily::new(
        "source-port-zero-product-sunset",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        [[1, 0, 0], [0, 0, 1], [1, 2, 1]]
            .into_iter()
            .map(|row| {
                AffineDenominator::new(
                    -mass.clone(),
                    row.into_iter()
                        .map(|value| context.integer(value))
                        .collect(),
                )
            })
            .collect(),
        Vec::new(),
        vec![context.zero(); 3],
    )
    .unwrap();
    let zeros: Arc<[[bool; 3]]> = Arc::from([
        [false, false, false],
        [true, false, false],
        [false, true, false],
        [false, false, true],
    ]);
    let sources = SourceSystem::from_family(&family).unwrap();
    let audit = SourcePortAudit::try_new(&family, zeros.clone()).unwrap();
    let mut reports = Vec::new();
    for sector in [
        [false, true, true],
        [true, false, true],
        [true, true, false],
        [true, true, true],
    ] {
        let solver = SectorSolver::new(
            &sources,
            sector,
            SectorConfig {
                zero_sectors: zeros.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        let solution = solver.solve_sector(SectorSolveOptions::default()).unwrap();
        let report = audit.audit_sector(sector, None, &solution).unwrap();
        assert!(report.issues.is_empty(), "{sector:?}: {:?}", report.issues);
        assert_eq!(report.exact_replayed_rules, report.rules);
        assert_eq!(report.uniformly_descending_rules, report.rules);
        assert_eq!(report.checked_rule_uncovered_boxes, 0);
        assert_eq!(report.additional_replay_guard_branches, 0);
        reports.push(report);
    }
    assert_eq!(reports.iter().map(|report| report.rules).sum::<usize>(), 18);
    audit.validate_sector_census(&reports).unwrap();
}

#[test]
fn last_three_vacuum_replay_sectors_close_via_weighted_original_product_certificates() {
    let context = CoefficientContext::new(["d", "m"]);
    let mass = context.parameter("m").unwrap();
    let momenta: [[i64; 3]; 6] = [
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 1, 0],
        [1, 0, 1],
        [0, 1, -1],
    ];
    let family = IntegralFamily::new(
        "source-port-weighted-zero-vacuum",
        (0..3).map(|i| format!("k{i}")).collect(),
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        momenta
            .into_iter()
            .map(|momentum| {
                AffineDenominator::new(
                    -mass.clone(),
                    (0..3)
                        .flat_map(|i| {
                            (i..3).map(move |j| {
                                momentum[i] * momentum[j] * if i == j { 1 } else { 2 }
                            })
                        })
                        .map(|value| context.integer(value))
                        .collect(),
                )
            })
            .collect(),
        Vec::new(),
        vec![context.zero(); 6],
    )
    .unwrap();
    let analyzer = crate::sector::zero::Analyzer::try_unrestricted(&family).unwrap();
    let zeros: Arc<[[bool; 6]]> = Arc::from(
        (0..64)
            .map(|bits| std::array::from_fn(|axis| bits & (1 << axis) != 0))
            .filter(|mask| {
                matches!(
                    analyzer
                        .analyze(&crate::sector::Mask::try_new(*mask).unwrap())
                        .unwrap(),
                    crate::sector::zero::Decision::ProvedZero(_)
                )
            })
            .collect::<Vec<_>>(),
    );
    let sources = SourceSystem::from_family(&family).unwrap();
    let audit = SourcePortAudit::try_new(&family, zeros.clone()).unwrap();
    for sector in [
        [false, false, true, true, false, true],
        [false, true, false, false, true, true],
        [true, false, false, false, true, true],
    ] {
        let solver = SectorSolver::new(
            &sources,
            sector,
            SectorConfig {
                zero_sectors: zeros.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        let solution = solver.solve_sector(SectorSolveOptions::default()).unwrap();
        let report = audit.audit_sector(sector, None, &solution).unwrap();
        assert!(report.issues.is_empty(), "{sector:?}: {:?}", report.issues);
        assert_eq!(report.exact_replayed_rules, report.rules);
        assert_eq!(report.uniformly_descending_rules, report.rules);
        assert_eq!(report.additional_replay_guard_branches, 0);
        assert_eq!(report.checked_rule_uncovered_boxes, 0);
    }
}
