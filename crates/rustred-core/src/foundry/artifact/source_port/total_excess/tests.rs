use super::super::tests::{solved_tadpole, tadpole};
use super::*;
use crate::algebra::CoefficientContext;
use crate::foundry::artifact::source_port::certificate::{
    OriginalRowNormalization, OriginalSourceReplay,
};
use crate::foundry::artifact::source_port::program::CheckedRhs;

#[path = "tests/successor_reuse.rs"]
mod successor_reuse;

#[test]
fn real_checked_tadpole_reports_exact_entry_and_successor_degree() {
    let (audit, solution) = solved_tadpole();
    let report = audit
        .audit_complete_through_total_excess(&tadpole(), [([true], None, solution)], 3)
        .unwrap();
    assert_eq!(report.max_entry_total_excess_degree(), 3);
    assert_eq!(report.max_successor_total_excess_degree(), 3);
    assert_eq!(report.successor_degrees()[&[true]], 3);
    assert!(report.contains_entry(&[4]).unwrap());
    assert!(!report.contains_entry(&[5]).unwrap());
    assert!(report.contains_entry(&[-3]).unwrap());
    assert!(!report.contains_entry(&[-4]).unwrap());
    assert_eq!(report.sectors()[0].max_total_excess_degree, Some(3));
    assert_eq!(report.sectors()[0].exact_replayed_rules, 1);
    assert!(report.sectors()[0].issues.is_empty());
}

#[test]
fn complete_degree_audit_rejects_bad_census_and_guarded_coverage_gaps() {
    let (audit, _) = solved_tadpole();
    assert!(
        audit
            .audit_complete_through_total_excess(&tadpole(), [], 2)
            .is_err()
    );
    let (audit, mut solution) = solved_tadpole();
    solution.finite_residuals.clear();
    assert!(
        audit
            .audit_complete_through_total_excess(&tadpole(), [([true], None, solution)], 0,)
            .unwrap_err()
            .to_string()
            .contains("does not cover successor total excess 0")
    );
}

#[test]
fn degree_audit_keeps_full_original_source_replay() {
    let (audit, mut solution) = solved_tadpole();
    let coefficient = &solution.rules[0].candidate.rhs[0].coefficient;
    solution.rules[0].candidate.rhs[0].coefficient = coefficient + coefficient;
    assert!(
        audit
            .audit_complete_through_total_excess(&tadpole(), [([true], None, solution)], 0,)
            .is_err(),
        "even an out-of-entry rule retains full original replay"
    );
}

#[test]
fn total_excess_geometry_budget_is_cumulative_and_never_reset() {
    let mut limits = CompletionGeometryLimits::default();
    limits.max_requested_boxes = 7;
    let mut budget = EnvelopeBudget::new(limits);
    let cell = LatticeBox::try_new([0], [None]).unwrap();
    assert_eq!(
        budget
            .partition(&cell, &[true], &[-1])
            .unwrap()
            .as_slice()
            .len(),
        2
    );
    assert!(budget.partition(&cell, &[true], &[-1]).is_err());
    assert!(budget.partition(&cell, &[true], &[-1]).is_err());
    let (audit, solution) = solved_tadpole();
    let mut limits = super::super::SourcePortLimits::default();
    limits.cover_replay.max_requested_boxes = 0;
    assert!(matches!(
        audit
            .with_limits(limits)
            .audit_complete_through_total_excess(&tadpole(), [([true], None, solution)], 2,),
        Err(SourcePortAuditError::ResourceBudgetExhausted { .. })
    ));
}

#[test]
fn shift_bound_covers_actual_degree_increases_and_extreme_arithmetic() {
    let parent = [-29, 0, 0, 1, 1, 1, -1, 1, 1, 0];
    let child = [-29, 0, -1, 1, 1, 2, -1, 0, 1, 0];
    let shift = std::array::from_fn::<_, 10, _>(|axis| child[axis] - parent[axis]);
    assert_eq!(excess(&parent), 30);
    assert_eq!(excess(&child), 32);
    assert_eq!(
        successor_degree(30, &parent.map(|n| n > 0), &child.map(|n| n > 0), &shift).unwrap(),
        34
    );
    assert!(successor_degree(u64::MAX, &[true], &[false], &[-1]).is_err());
    assert_eq!(
        successor_degree(0, &[true], &[false], &[i64::MIN]).unwrap(),
        (1_u64 << 63) + 1
    );
    for a in -3..=3 {
        for b in -3..=3 {
            let parent = [a, b];
            for x in -2..=2 {
                for y in -2..=2 {
                    let child = [a + x, b + y];
                    let bound = successor_degree(
                        excess(&parent),
                        &parent.map(|n| n > 0),
                        &child.map(|n| n > 0),
                        &[x, y],
                    )
                    .unwrap();
                    assert!(excess(&child) <= bound);
                }
            }
        }
    }
}

fn excess(powers: &[i64]) -> u64 {
    powers
        .iter()
        .map(|&n| {
            if n > 0 {
                (n - 1) as u64
            } else {
                n.unsigned_abs()
            }
        })
        .sum()
}

// These synthetic rules test only the conservative graph arithmetic. They
// cannot enter the public diagnostic, which reconstructs CheckedRules solely
// after real original-source, guards and descent checks.
fn rule<const N: usize>(shift: [i64; N], application: LatticeBox) -> CheckedRule<N> {
    let context = CoefficientContext::new(
        std::iter::once("d".to_owned()).chain((0..N).map(|axis| format!("n{axis}"))),
    );
    CheckedRule {
        fixed: [None; N],
        rhs: vec![CheckedRhs {
            shift,
            coefficient: context.one(),
        }],
        application: vec![application],
        nonzero_conditions: Vec::new(),
        ordinary: OriginalSourceReplay {
            normalization: OriginalRowNormalization::OriginalGeneratorOrdinaryV1,
            contributions: Vec::new(),
            source_conditions: Vec::new(),
        },
        affine: None,
        affine_exclusions: Vec::new(),
    }
}

#[test]
fn dag_propagation_uses_actual_sector_order_and_distinct_entry_degree() {
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let entry = EntryScope::try_new(
        &family,
        &Mask::try_new([true; 3]).unwrap(),
        EntryDegreeBound::MaxTotalExcessDegree(3),
    )
    .unwrap();
    let ordering = OrderingPolicy::SpiredUncutV1;
    // Lexical mask order is the opposite of physical sector complexity here.
    let parent = [false, true, true];
    let child = [true, false, false];
    assert!(parent < child);
    assert_eq!(
        ordering
            .compare(&parent.map(i64::from), &child.map(i64::from))
            .unwrap(),
        Ordering::Greater
    );
    let singleton = LatticeBox::try_new([0; 3], [Some(0); 3]).unwrap();
    let rule = rule([1, -1, -1], singleton);
    let mut bounds = BTreeMap::from([(parent, 3), (child, 3)]);
    propagate_rule(
        &entry,
        ordering,
        parent,
        3,
        &rule,
        &[],
        &[1, 2, 3],
        &mut bounds,
        &mut EnvelopeBudget::new(CompletionGeometryLimits::default()),
    )
    .unwrap();
    assert_eq!(bounds[&child], 7);
    assert_eq!(entry.bound().limit(), 3);
}

#[test]
fn exact_degree_intersection_excludes_only_empty_sign_pieces() {
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let entry = EntryScope::try_new(
        &family,
        &Mask::try_new([true; 3]).unwrap(),
        EntryDegreeBound::MaxTotalExcessDegree(3),
    )
    .unwrap();
    let source = LatticeBox::try_new([2, 2, 0], [Some(2), Some(2), Some(0)]).unwrap();
    // Every individual coordinate fits 3, but the exact degree sum is 4.
    let rule = rule([1, -3, -1], source);
    let parent = [false, true, true];
    let child = [true, false, false];
    let mut bounds = BTreeMap::from([(parent, 3), (child, 3)]);
    propagate_rule(
        &entry,
        OrderingPolicy::SpiredUncutV1,
        parent,
        3,
        &rule,
        &[],
        &[1, 2, 3],
        &mut bounds,
        &mut EnvelopeBudget::new(CompletionGeometryLimits::default()),
    )
    .unwrap();
    assert_eq!(bounds[&child], 3);
}

#[test]
fn suspect_edges_require_exact_zero_or_admitted_lower_destination() {
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let parent = [true, false, true];
    let entry = EntryScope::try_new(
        &family,
        &Mask::try_new(parent).unwrap(),
        EntryDegreeBound::MaxTotalExcessDegree(2),
    )
    .unwrap();
    let outside_lower = [false, true, false];
    let harder = [true; 3];
    let context = CoefficientContext::new(["d", "n0", "n1", "n2"]);
    for (shift, child) in [([-1, 1, -1], outside_lower), ([0, 1, 0], harder)] {
        let singleton = LatticeBox::try_new([0; 3], [Some(0); 3]).unwrap();
        let mut rule = rule(shift, singleton);
        let mut bounds = BTreeMap::from([(parent, 2)]);
        let mut propagate = |rule: &CheckedRule<3>, zeros: &[[bool; 3]]| {
            propagate_rule(
                &entry,
                OrderingPolicy::SpiredUncutV1,
                parent,
                2,
                rule,
                zeros,
                &[1, 2, 3],
                &mut bounds,
                &mut EnvelopeBudget::new(CompletionGeometryLimits::default()),
            )
        };
        assert!(
            propagate(&rule, &[])
                .unwrap_err()
                .to_string()
                .contains("nonlower or out-of-root")
        );
        // In the public path this list comes solely from the independent zero
        // audit. Even an out-of-root zero image is exactly zero and stops.
        propagate(&rule, &[child]).unwrap();
        // n1=0 on this complete sign piece. This exact algebra proof removes
        // an apparent activation; a merely small/nonzero coefficient cannot.
        rule.rhs[0].coefficient = context.coefficient_fixture("n1");
        propagate(&rule, &[]).unwrap();
        rule.rhs[0].coefficient = context.coefficient_fixture("n1+1");
        assert!(propagate(&rule, &[]).is_err());
        assert_eq!(bounds, BTreeMap::from([(parent, 2)]));
    }
}

#[test]
fn same_sector_checked_descent_does_not_charge_a_shift_growth_cycle() {
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let sector = [true, true, false];
    let entry = EntryScope::try_new(
        &family,
        &Mask::try_new([true; 3]).unwrap(),
        EntryDegreeBound::MaxTotalExcessDegree(3),
    )
    .unwrap();
    let application = LatticeBox::try_new([2, 0, 0], [Some(2), Some(0), Some(0)]).unwrap();
    let rule = rule([-1, 0, 0], application);
    let mut bounds = BTreeMap::from([(sector, 3)]);
    propagate_rule(
        &entry,
        OrderingPolicy::SpiredUncutV1,
        sector,
        3,
        &rule,
        &[],
        &[1, 2, 3],
        &mut bounds,
        &mut EnvelopeBudget::new(CompletionGeometryLimits::default()),
    )
    .unwrap();
    assert_eq!(bounds[&sector], 3);
}

#[test]
fn real_k3_complete_audit_propagates_before_checking_each_child() {
    use crate::solver::{SectorConfig, SectorSolveOptions, SectorSolver, SourceSystem};
    use std::sync::Arc;
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let zeros: Arc<[[bool; 3]]> = Arc::from([
        [false; 3],
        [true, false, false],
        [false, true, false],
        [false, false, true],
    ]);
    let sources = SourceSystem::from_family(&family).unwrap();
    let audit = SourcePortAudit::try_new(&family, zeros.clone()).unwrap();
    // Deliberately supply an arbitrary order with the full parent last.
    let solutions = [
        [true, false, true],
        [false, true, true],
        [true, true, false],
        [true; 3],
    ]
    .into_iter()
    .map(|sector| {
        let solution = SectorSolver::new(
            &sources,
            sector,
            SectorConfig {
                zero_sectors: zeros.clone(),
                ..Default::default()
            },
        )
        .unwrap()
        .solve_sector(SectorSolveOptions::default())
        .unwrap();
        (sector, None, solution)
    });
    let mut checked_order = Vec::new();
    let prepared = audit
        .prepare_complete_through_total_excess_with_observer(&family, solutions, 2, &mut |event| {
            if let SourcePortInstallEvent::CheckedSector { report, .. } = event {
                checked_order.push((report.sector, report.max_total_excess_degree.unwrap()));
            }
        })
        .unwrap();
    assert_eq!(prepared.sectors.len(), 4);
    assert_eq!(
        prepared.sectors.keys().collect::<Vec<_>>(),
        prepared
            .report
            .successor_degrees()
            .keys()
            .collect::<Vec<_>>()
    );
    let report = prepared.report;
    assert_eq!(report.sectors().len(), 4);
    assert_eq!(checked_order[0], ([true; 3], 2));
    assert!(report.max_successor_total_excess_degree() > 2);
    for pair in checked_order.windows(2) {
        assert_eq!(
            report
                .ordering()
                .compare(&pair[0].0.map(i64::from), &pair[1].0.map(i64::from))
                .unwrap(),
            Ordering::Greater
        );
    }
    for (sector, degree) in checked_order {
        assert_eq!(report.successor_degrees()[&sector], degree);
    }
    assert!(report.contains_entry(&[3, 1, 1]).unwrap());
    assert!(!report.contains_entry(&[4, 1, 1]).unwrap());
    assert!(report.contains_entry(&[2, -1, 1]).unwrap());
    assert!(!report.contains_entry(&[3, -1, 1]).unwrap());
}

#[test]
fn census_and_priority_fail_before_untrusted_sector_rules_are_checked() {
    use std::sync::Arc;
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let zeros: Arc<[[bool; 3]]> = Arc::from([
        [false; 3],
        [true, false, false],
        [false, true, false],
        [false, false, true],
    ]);
    let audit = SourcePortAudit::try_new(&family, zeros).unwrap();
    let empty = || SectorSolution {
        finite_case_policy: Default::default(),
        max_numerator_rank: None,
        rules: Vec::new(),
        finite_residuals: Vec::new(),
        stats: Default::default(),
    };
    let duplicate = audit
        .audit_complete_through_total_excess(
            &family,
            [([true; 3], None, empty()), ([true; 3], None, empty())],
            2,
        )
        .unwrap_err();
    assert!(duplicate.to_string().contains("duplicate"));
    let mismatch = audit
        .audit_complete_through_total_excess(
            &family,
            [
                ([true; 3], None, empty()),
                ([true, true, false], Some([2, 1, 0]), empty()),
            ],
            2,
        )
        .unwrap_err();
    assert!(
        mismatch
            .to_string()
            .contains("incompatible coordinate priorities")
    );
}
