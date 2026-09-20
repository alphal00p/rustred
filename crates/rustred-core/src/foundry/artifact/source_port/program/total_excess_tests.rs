//! The retained proof is a private intermediate, not a bounded artifact.

use super::super::tests::{solved_tadpole, tadpole};
use super::*;

fn retain<const N: usize>(
    audit: SourcePortAudit<N>,
    family: IntegralFamily,
    sectors: impl IntoIterator<Item = ([bool; N], Option<[usize; N]>, SectorSolution<N>)>,
    degree: u64,
) -> Result<CheckedProgram<N>, SourcePortAuditError> {
    audit.retain_total_excess_program_with_observer(family, sectors, degree, &mut |_| {})
}

fn assert_coefficient(left: &Coefficient, right: &Coefficient) {
    assert_eq!(left, right);
    assert_eq!(left.numerator.variables(), right.numerator.variables());
    assert_eq!(left.denominator.variables(), right.denominator.variables());
}

#[test]
fn bounded_retention_keeps_original_rules_maps_guards_and_owners() {
    let (audit, solution) = solved_tadpole();
    let unrestricted = audit
        .retain_program(tadpole(), [([true], None, solution)])
        .unwrap();
    assert!(matches!(
        unrestricted.scope,
        CheckedProgramScope::Unrestricted
    ));
    let (audit, solution) = solved_tadpole();
    let bounded = retain(audit, tadpole(), [([true], None, solution)], 3).unwrap();
    let CheckedProgramScope::TotalExcess(report) = &bounded.scope else {
        panic!("retention must preserve the bounded promise");
    };
    assert_eq!(report.family_fingerprint(), bounded.family.fingerprint());
    assert_eq!(report.root_sector(), &bounded.root_sector);
    assert_eq!(report.ordering(), bounded.ordering);
    assert_eq!(report.max_entry_total_excess_degree(), 3);
    assert_eq!(report.successor_degrees(), &BTreeMap::from([([true], 3)]));
    assert_eq!(report.sectors().len(), bounded.sectors.len());
    assert_eq!(bounded.limits, unrestricted.limits);
    assert_eq!(bounded.zero_sectors, unrestricted.zero_sectors);
    assert_eq!(
        bounded.original_sources.family_fingerprint(),
        bounded.family.fingerprint()
    );
    assert_eq!(
        bounded.original_sources.context().fingerprint(),
        unrestricted.original_sources.context().fingerprint()
    );
    assert_coefficient(
        bounded.original_sources.context().one().raw(),
        unrestricted.original_sources.context().one().raw(),
    );
    assert_eq!(
        bounded.inherited_source_conditions,
        unrestricted.inherited_source_conditions
    );

    let actual = &bounded.sectors[&[true]];
    let expected = &unrestricted.sectors[&[true]];
    assert_eq!(actual.terminals, expected.terminals);
    assert_eq!(actual.rules.len(), report.sectors()[0].exact_replayed_rules);
    assert_eq!(actual.rules.len(), expected.rules.len());
    for (actual, expected) in actual.rules.iter().zip(&expected.rules) {
        assert_eq!(actual.fixed, expected.fixed);
        assert_eq!(actual.application, expected.application);
        assert_eq!(actual.nonzero_conditions, expected.nonzero_conditions);
        assert_eq!(
            actual.ordinary.normalization,
            expected.ordinary.normalization
        );
        assert_eq!(
            actual.ordinary.source_conditions,
            expected.ordinary.source_conditions
        );
        assert_eq!(
            actual.ordinary.contributions.len(),
            expected.ordinary.contributions.len()
        );
        for (actual, expected) in actual
            .ordinary
            .contributions
            .iter()
            .zip(&expected.ordinary.contributions)
        {
            assert_eq!(actual.source_row, expected.source_row);
            assert_eq!(actual.offset, expected.offset);
            assert_coefficient(&actual.weight, &expected.weight);
        }
        assert_eq!(actual.rhs.len(), expected.rhs.len());
        for (actual, expected) in actual.rhs.iter().zip(&expected.rhs) {
            assert_eq!(actual.shift, expected.shift);
            assert_coefficient(&actual.coefficient, &expected.coefficient);
        }
    }
    assert!(
        bounded
            .install()
            .unwrap_err()
            .to_string()
            .contains("scoped lowering and final cell admission")
    );
}

#[test]
fn bounded_retention_has_only_the_existing_ordered_check_events() {
    let (audit, solution) = solved_tadpole();
    let caller = std::thread::current().id();
    let mut events = Vec::new();
    let program = audit
        .retain_total_excess_program_with_observer(
            tadpole(),
            [([true], None, solution)],
            2,
            &mut |event| {
                assert_eq!(caller, std::thread::current().id());
                events.push(match event {
                    SourcePortInstallEvent::CheckingSector {
                        ordinal: 0,
                        sector: [true],
                        rules: 1,
                        ..
                    } => "sector",
                    SourcePortInstallEvent::CheckingRule {
                        ordinal: 0,
                        sector: [true],
                        total: 1,
                        ..
                    } => "rule",
                    SourcePortInstallEvent::CheckedSector {
                        ordinal: 0, report, ..
                    } => {
                        assert_eq!(report.max_total_excess_degree, Some(2));
                        "checked"
                    }
                    _ => panic!("retention must not lower or install"),
                });
            },
        )
        .unwrap();
    assert_eq!(events, ["sector", "rule", "checked"]);
    assert_eq!(program.sectors.len(), 1);
}

#[test]
fn bounded_retention_rejects_incomplete_or_corrupt_original_evidence() {
    for corrupt_identity in [false, true] {
        let (audit, mut solution) = solved_tadpole();
        if corrupt_identity {
            solution.rules[0].candidate.rhs[0].coefficient =
                -solution.rules[0].candidate.rhs[0].coefficient.clone();
        } else {
            solution.finite_residuals.clear();
        }
        // E=0 must not let an out-of-entry rule escape original-source replay.
        assert!(retain(audit, tadpole(), [([true], None, solution)], 0).is_err());
    }
    let (audit, mut solution) = solved_tadpole();
    solution.rules[0].exceptions = Default::default();
    let program = retain(audit, tadpole(), [([true], None, solution)], 2).unwrap();
    let rule = &program.sectors[&[true]].rules[0];
    assert!(rule.application.iter().all(|cell| cell.lower()[0] >= 1));
    assert!(!rule.nonzero_conditions.is_empty());
}

#[test]
fn bounded_retention_rejects_census_and_order_before_check_events() {
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let zeros: Arc<[[bool; 3]]> = Arc::from([
        [false; 3],
        [true, false, false],
        [false, true, false],
        [false, false, true],
    ]);
    let empty = || SectorSolution {
        rules: Vec::new(),
        finite_residuals: Vec::new(),
        stats: Default::default(),
    };
    for (sectors, expected) in [
        (vec![([true; 3], None, empty())], "does not exhaust"),
        (
            vec![([true; 3], None, empty()), ([true; 3], None, empty())],
            "duplicate",
        ),
        (vec![([false; 3], None, empty())], "zero solved sector"),
        (
            vec![
                ([true; 3], None, empty()),
                ([true, true, false], Some([2, 1, 0]), empty()),
            ],
            "incompatible coordinate priorities",
        ),
    ] {
        let audit = SourcePortAudit::try_new(&family, zeros.clone()).unwrap();
        let owned_family =
            crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
        assert_eq!(owned_family.fingerprint(), family.fingerprint());
        let Err(error) =
            audit.retain_total_excess_program_with_observer(owned_family, sectors, 2, &mut |_| {
                panic!("invalid census/order must precede replay")
            })
        else {
            panic!("invalid census/order was retained");
        };
        assert!(error.to_string().contains(expected), "{error}");
    }
    let audit =
        SourcePortAudit::try_new_with_root_sector(&family, zeros, [true, true, false]).unwrap();
    let Err(error) = retain(audit, family, [([true; 3], None, empty())], 2) else {
        panic!("out-of-root sector was retained");
    };
    assert!(error.to_string().contains("outside the entry root"));
}

#[test]
fn bounded_retention_checks_family_and_caller_limits_before_consuming_input() {
    let (audit, _) = solved_tadpole();
    let foreign = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let never_read = || {
        std::iter::once_with(|| -> ([bool; 1], Option<[usize; 1]>, SectorSolution<1>) {
            panic!("invalid binding or policy must not consume sectors")
        })
    };
    let Err(error) = retain(audit, foreign, never_read(), 2) else {
        panic!("wrong family was retained");
    };
    assert!(
        error
            .to_string()
            .contains("family differs from original sources")
    );
    let (audit, _) = solved_tadpole();
    let audit = audit.with_limits(SourcePortLimits {
        max_predicate_atoms: 257,
        ..Default::default()
    });
    assert!(matches!(
        retain(audit, tadpole(), never_read(), 2),
        Err(SourcePortAuditError::UnsupportedResourcePolicy { .. })
    ));
    let (audit, _) = solved_tadpole();
    let mut limits = SourcePortLimits::default();
    limits.cover_replay.max_requested_boxes = 0;
    assert!(matches!(
        retain(audit.with_limits(limits), tadpole(), never_read(), 2),
        Err(SourcePortAuditError::ResourceBudgetExhausted {
            resource: "total-excess successor geometry"
        })
    ));
}
