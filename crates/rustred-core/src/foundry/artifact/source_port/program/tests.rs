use super::super::tests::{solved_tadpole, tadpole};
use super::*;
use crate::sector::{CoordinatePriority, CoordinatePriorityLimits, Mask, zero};
use crate::solver::{SectorConfig, SectorSolveOptions, SectorSolver, SourceSystem};

#[test]
fn source_port_rule_policy_survives_retention_and_matches_cold_loading() {
    use crate::foundry::artifact::{ArtifactLoadLimits, ClosedArtifact};

    let (audit, solution) = solved_tadpole();
    let baseline = audit
        .install_complete(tadpole(), [([true], None, solution)])
        .unwrap()
        .encode_durable()
        .unwrap();
    assert_eq!(
        SourcePortLimits::default().max_predicate_consistency_work,
        4_194_304
    );
    assert_eq!(
        SourcePortLimits::default()
            .rule_derivation
            .max_domain_bound_endpoint_cells,
        8_192
    );
    assert_eq!(
        ArtifactLoadLimits::default().max_predicate_consistency_work,
        SourcePortLimits::default().max_predicate_consistency_work
    );
    for endpoints in [13, 14, 16_384] {
        let (audit, solution) = solved_tadpole();
        let mut limits = SourcePortLimits::default();
        limits.rule_derivation.max_domain_bound_endpoint_cells = endpoints;
        // This coordinate-only proof needs no affine native consistency work.
        limits.max_predicate_consistency_work = 0;
        let program = audit
            .with_limits(limits)
            .retain_program(tadpole(), [([true], None, solution)])
            .unwrap();
        assert_eq!(program.limits, limits);
        let result = program.install();
        let cold_limits = ArtifactLoadLimits {
            rule_derivation: limits.rule_derivation,
            max_predicate_consistency_work: 0,
            ..Default::default()
        };
        let cold = ClosedArtifact::decode_durable_with_limits(&baseline, cold_limits);
        if endpoints == 13 {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("combined domain bound endpoint cells")
            );
            assert!(
                cold.unwrap_err()
                    .to_string()
                    .contains("combined domain bound endpoint cells")
            );
        } else {
            assert_eq!(result.unwrap().encode_durable().unwrap(), baseline);
            assert_eq!(cold.unwrap().encode_durable().unwrap(), baseline);
        }
    }
    let (audit, solution) = solved_tadpole();
    assert_eq!(
        audit
            .with_limits(SourcePortLimits::default())
            .install_complete(tadpole(), [([true], None, solution)])
            .unwrap()
            .encode_durable()
            .unwrap(),
        baseline
    );
}

#[test]
fn install_observer_preserves_durable_bytes_and_runs_on_calling_thread() {
    let (audit, solution) = solved_tadpole();
    let baseline = audit
        .install_complete(tadpole(), [([true], None, solution)])
        .unwrap();
    let (audit, solution) = solved_tadpole();
    let explicit_noop = audit
        .install_complete_with_observer(tadpole(), [([true], None, solution)], |_| {})
        .unwrap();
    assert_eq!(
        baseline.encode_durable().unwrap(),
        explicit_noop.encode_durable().unwrap()
    );

    let caller = std::thread::current().id();
    let mut events = Vec::new();
    let (audit, solution) = solved_tadpole();
    let observed = audit
        .install_complete_with_observer(tadpole(), [([true], None, solution)], |event| {
            assert_eq!(std::thread::current().id(), caller);
            let (name, elapsed) = match event {
                SourcePortInstallEvent::CheckingSector {
                    ordinal,
                    sector,
                    rules,
                    elapsed,
                } => {
                    assert_eq!((ordinal, sector, rules), (0, [true], 1));
                    ("checking", elapsed)
                }
                SourcePortInstallEvent::CheckingRule {
                    sector,
                    ordinal,
                    total,
                    elapsed,
                } => {
                    assert_eq!((sector, ordinal, total), ([true], 0, 1));
                    ("checking rule", elapsed)
                }
                SourcePortInstallEvent::CheckedSector {
                    ordinal,
                    report,
                    elapsed,
                } => {
                    assert_eq!(
                        (ordinal, report.sector, report.exact_replayed_rules),
                        (0, [true], 1)
                    );
                    assert!(report.issues.is_empty());
                    ("checked", elapsed)
                }
                SourcePortInstallEvent::LoweringRule {
                    sector,
                    ordinal,
                    total,
                    elapsed,
                } => {
                    assert_eq!((sector, ordinal, total), ([true], 0, 1));
                    ("lowering", elapsed)
                }
                SourcePortInstallEvent::LoweredSector {
                    sector,
                    cells,
                    elapsed,
                } => {
                    assert_eq!(sector, [true]);
                    assert_eq!(cells, baseline.rule_cells().len());
                    ("lowered", elapsed)
                }
                SourcePortInstallEvent::Installing {
                    sectors,
                    rule_cells,
                    terminals,
                    elapsed,
                } => {
                    assert_eq!(sectors, 1);
                    assert_eq!(rule_cells, baseline.rule_cells().len());
                    assert_eq!(terminals, baseline.masters().len());
                    ("installing", elapsed)
                }
                SourcePortInstallEvent::Installed { elapsed } => ("installed", elapsed),
            };
            events.push((name, elapsed));
            // Observable caller work may perturb timings, never rule choices.
            std::thread::yield_now();
        })
        .unwrap();
    assert_eq!(
        events.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        [
            "checking",
            "checking rule",
            "checked",
            "lowering",
            "lowered",
            "installing",
            "installed"
        ]
    );
    assert!(events.windows(2).all(|pair| pair[0].1 <= pair[1].1));
    assert_eq!(
        baseline.encode_durable().unwrap(),
        observed.encode_durable().unwrap()
    );
}

#[test]
fn observed_incomplete_sector_cannot_reach_lowering_or_installation() {
    let (audit, mut solution) = solved_tadpole();
    solution.finite_residuals.clear();
    let baseline = audit
        .install_complete(tadpole(), [([true], None, solution)])
        .unwrap_err()
        .to_string();
    let (audit, mut solution) = solved_tadpole();
    solution.finite_residuals.clear();
    let mut events = Vec::new();
    let failure =
        audit
            .install_complete_with_observer(tadpole(), [([true], None, solution)], |event| {
                match event {
                    SourcePortInstallEvent::CheckingSector { .. } => events.push("checking"),
                    SourcePortInstallEvent::CheckingRule { ordinal, total, .. } => {
                        assert_eq!((ordinal, total), (0, 1));
                        events.push("checking rule");
                    }
                    SourcePortInstallEvent::CheckedSector { report, .. } => {
                        assert!(report.checked_rule_uncovered_boxes > 0);
                        events.push("incomplete report");
                    }
                    _ => panic!("incomplete diagnostic was treated as admission evidence"),
                }
            })
            .unwrap_err();
    assert_eq!(events, ["checking", "checking rule", "incomplete report"]);
    assert_eq!(failure.to_string(), baseline);
}

#[test]
fn observer_panic_aborts_instead_of_creating_a_successful_artifact() {
    let (audit, solution) = solved_tadpole();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        audit.install_complete_with_observer(tadpole(), [([true], None, solution)], |event| {
            if matches!(event, SourcePortInstallEvent::CheckingRule { .. }) {
                panic!("observer requested no authority");
            }
        })
    }));
    assert!(outcome.is_err());
}

#[test]
fn rule_check_observations_follow_original_rule_order() {
    let (audit, mut solution) = solved_tadpole();
    let (_, second) = solved_tadpole();
    solution.rules.extend(second.rules);
    let caller = std::thread::current().id();
    let mut ordinals = Vec::new();
    let checked = audit
        .check_sector_with_observer([true], None, &solution, Instant::now(), &mut |event| {
            assert_eq!(std::thread::current().id(), caller);
            let SourcePortInstallEvent::CheckingRule {
                sector,
                ordinal,
                total,
                ..
            } = event
            else {
                panic!("the rule checker must only announce rule starts");
            };
            assert_eq!((sector, total), ([true], 2));
            ordinals.push(ordinal);
        })
        .unwrap();
    assert_eq!(ordinals, [0, 1]);
    assert_eq!(checked.report.exact_replayed_rules, 2);
    assert!(checked.report.issues.is_empty());
}

#[test]
fn checked_program_owns_original_replay_unbounded_cover_and_finite_terminals() {
    let (audit, solution) = solved_tadpole();
    assert_eq!(audit.proved_zero_sector_count(), 1);
    let family = tadpole();
    let fingerprint = family.fingerprint().to_owned();
    let program = audit
        .retain_program(family, [([true], None, solution)])
        .unwrap();
    assert_eq!(program.family.fingerprint(), fingerprint);
    assert_eq!(program.original_sources.family_fingerprint(), fingerprint);
    assert_eq!(program.ordering, OrderingPolicy::SpiredUncutV1);
    assert_eq!(program.zero_sectors.as_ref(), &[[false]]);
    let sector = &program.sectors[&[true]];
    assert_eq!(sector.sector, [true]);
    assert_eq!(sector.terminals, vec![[1_i64]]);
    assert_eq!(sector.rules.len(), 1);
    let rule = &sector.rules[0];
    assert_eq!(rule.fixed, [None]);
    assert_eq!(rule.rhs[0].shift, [-1_i64]);
    assert!(!rule.rhs[0].coefficient.is_zero());
    assert_eq!(
        rule.ordinary.normalization,
        OriginalRowNormalization::OriginalGeneratorOrdinaryV1
    );
    assert!(!rule.ordinary.contributions.is_empty());
    assert!(rule.application.iter().any(|cell| cell.upper() == [None]));
    assert!(!rule.nonzero_conditions.is_empty());
}

#[test]
fn incomplete_diagnostics_remain_available_but_cannot_be_retained() {
    for remove_rule in [false, true] {
        let (audit, mut solution) = solved_tadpole();
        if remove_rule {
            solution.rules.clear();
        } else {
            solution.finite_residuals.clear();
        }
        let report = audit.audit_sector([true], None, &solution).unwrap();
        assert!(report.checked_rule_uncovered_boxes > 0);
        assert_eq!(report.checked_rule_unbounded_boxes > 0, remove_rule);
        assert!(
            audit
                .retain_program(tadpole(), [([true], None, solution)])
                .is_err()
        );
    }
}

#[test]
fn mutated_identity_cannot_enter_the_private_program() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].candidate.rhs[0].coefficient =
        -solution.rules[0].candidate.rhs[0].coefficient.clone();
    assert!(
        audit
            .retain_program(tadpole(), [([true], None, solution)])
            .is_err()
    );
}

#[test]
fn missing_serialized_guard_is_recomputed_before_retention() {
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].exceptions = Default::default();
    let program = audit
        .retain_program(tadpole(), [([true], None, solution)])
        .unwrap();
    let rule = &program.sectors[&[true]].rules[0];
    assert!(rule.application.iter().all(|cell| cell.lower()[0] >= 1));
    assert!(!rule.nonzero_conditions.is_empty());
    assert_eq!(program.sectors[&[true]].terminals, vec![[1]]);
}

#[test]
fn complete_program_rejects_missing_duplicate_and_zero_solved_sectors() {
    let (audit, _) = solved_tadpole();
    let no_sectors = Vec::<([bool; 1], Option<[usize; 1]>, SectorSolution<1>)>::new();
    assert!(audit.retain_program(tadpole(), no_sectors).is_err());

    let (audit, first) = solved_tadpole();
    let (_, second) = solved_tadpole();
    assert!(
        audit
            .retain_program(tadpole(), [([true], None, first), ([true], None, second),])
            .is_err()
    );

    let (audit, solution) = solved_tadpole();
    assert!(
        audit
            .retain_program(tadpole(), [([false], None, solution)])
            .is_err()
    );
}

#[test]
fn foreign_family_binding_is_rejected_without_a_new_hash_layer() {
    let (audit, solution) = solved_tadpole();
    let foreign = crate::foundry::artifact::three_loop::canonical_family().unwrap();
    assert!(
        audit
            .retain_program(foreign, [([true], None, solution)])
            .is_err()
    );
}

#[test]
fn incompatible_sector_priorities_cannot_be_flattened_into_one_order() {
    let natural = OrderingPolicy::SpiredUncutV1;
    let priority =
        CoordinatePriority::try_new(3, &[2, 1, 0], CoordinatePriorityLimits::default()).unwrap();
    let reversed = OrderingPolicy::try_spired_with_coordinate_priority(&priority).unwrap();
    let mut order = None;
    retain_common_order(&mut order, natural).unwrap();
    retain_common_order(&mut order, natural).unwrap();
    assert!(retain_common_order(&mut order, reversed).is_err());
    assert_eq!(order, Some(natural));
}

/// Use the actual unit-mass parent convention that Vakint's artifact will own.
/// Rule counts are measured, not copied from the differently ordered vac3
/// reference family. Run this explicitly with the release test profile.
#[test]
#[ignore = "full canonical K6 release ownership regression"]
fn canonical_unit_mass_k6_retains_the_complete_generated_program() {
    let family = crate::foundry::artifact::three_loop::canonical_family().unwrap();
    let analyzer = zero::Analyzer::try_unrestricted(&family).unwrap();
    let mut zeros = Vec::new();
    let mut nonzeros = Vec::new();
    for bits in 0_u64..64 {
        let sector: [bool; 6] = std::array::from_fn(|axis| (bits >> axis) & 1 != 0);
        match analyzer.analyze(&Mask::try_new(sector).unwrap()).unwrap() {
            zero::Decision::ProvedZero(_) => zeros.push(sector),
            _ => nonzeros.push(sector),
        }
    }
    assert_eq!(zeros.len(), 26);
    assert_eq!(nonzeros.len(), 38);
    let zeros: Arc<[[bool; 6]]> = zeros.into();
    let sources = SourceSystem::from_family(&family).unwrap();
    let mut solved = Vec::new();
    for sector in nonzeros {
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
        solved.push((sector, None, solution));
    }
    let audit = SourcePortAudit::try_new(&family, zeros).unwrap();
    let program = audit.retain_program(family, solved).unwrap();
    let rules: usize = program
        .sectors
        .values()
        .map(|sector| sector.rules.len())
        .sum();
    let terminals: usize = program
        .sectors
        .values()
        .map(|sector| sector.terminals.len())
        .sum();
    assert_eq!(program.sectors.len(), 38);
    assert!(rules > 0 && terminals > 0);
    // Bind the actual retained basis, not merely its count, to the corner
    // catalog studied for this canonical fixture. This is not a restriction
    // on generic programs, whose finite terminals need not be undotted.
    for (mask, sector) in &program.sectors {
        let corner: [i64; 6] = std::array::from_fn(|axis| i64::from(mask[axis]));
        assert_eq!(sector.terminals, vec![corner], "terminal set for {mask:?}");
        eprintln!("canonical unit-mass K6 terminal: {corner:?}");
    }
    eprintln!(
        "canonical unit-mass K6 checked program: sectors={}, rules={}, finite_terminals={}",
        program.sectors.len(),
        rules,
        terminals
    );
}
