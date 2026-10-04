use std::sync::Arc;

use super::super::super::tests::{solved_tadpole, tadpole};
use super::*;
use crate::foundry::artifact::{
    OriginalSourceCombinationRequest, check_original_source_combination,
};

#[test]
fn selected_circuit_keeps_normalized_weights_actual_recenter_and_borrowed_zero_proofs() {
    let (audit, solution) = solved_tadpole();
    let before = audit
        .replay_sector_rule_batch([true], None, &solution, &[0])
        .unwrap();
    let observed = audit
        .replay_sector_rule_circuits([true], None, &solution, &[0], Default::default())
        .unwrap();
    assert_eq!(observed.family_fingerprint(), tadpole().fingerprint());
    assert!(std::ptr::eq(
        observed.context(),
        audit.original_sources.context()
    ));
    assert!(std::ptr::eq(
        observed.zero_certificates(),
        audit.zero_certificates.as_slice()
    ));
    let rule = &observed.circuits()[0];
    assert_eq!(rule.ordinal(), 0);
    assert_eq!(
        rule.contributions().len(),
        before.rules[0].original_source_entries
    );
    assert_eq!(rule.contributions()[0].offset.values(), [-1]);
    assert_eq!(rule.raw_recenter()[0], -rule.raw_target()[0].value());
    assert_ne!(rule.raw_recenter()[0], 0);
    assert_eq!(rule.declared_case(), &solution.rules[0].candidate.case);
    assert_eq!(rule.declared_exceptions(), &solution.rules[0].exceptions);
    assert!(!rule.nonzero_conditions().is_empty());
    for denominator in rule
        .contributions()
        .iter()
        .map(|s| &s.weight.raw().denominator)
        .chain(rule.rhs().iter().map(|(_, c)| &c.raw().denominator))
    {
        assert!(
            rule.nonzero_conditions()
                .iter()
                .any(|p| p.raw() == denominator)
        );
    }
    let (lower, upper) = rule.application_boxes().next().unwrap();
    let proposal = OriginalSourceCombinationRequest {
        root_sector: observed.root_sector().clone(),
        sector: Mask::try_new(*observed.sector()).unwrap(),
        ordering: observed.ordering().clone(),
        lower: lower.to_vec(),
        upper: upper.to_vec(),
        fixed: Vec::new(),
        contributions: rule.contributions().to_vec(),
        rhs: rule.rhs().to_vec(),
        retained_conditions: rule.nonzero_conditions().to_vec(),
    };
    // Getter copies have no authority. Only a fresh original-producer check
    // can mint its own local proof, which happens to succeed for this fixture.
    check_original_source_combination(&tadpole(), proposal.clone(), Default::default()).unwrap();
    let mut bad = proposal.clone();
    bad.contributions[0].weight = observed.context().integer(42);
    assert!(check_original_source_combination(&tadpole(), bad, Default::default()).is_err());
    let mut bad = proposal.clone();
    bad.contributions[0].offset = IntegralShift::try_new([0]).unwrap();
    assert!(check_original_source_combination(&tadpole(), bad, Default::default()).is_err());
    let mut bad = proposal;
    bad.lower[0] = 0;
    bad.retained_conditions.clear();
    assert!(
        check_original_source_combination(&tadpole(), bad, Default::default()).is_err(),
        "omitting retained poles cannot make the n=1 weight pole admissible"
    );
    let after = audit
        .replay_sector_rule_batch([true], None, &solution, &[0])
        .unwrap();
    assert_eq!(before.rules, after.rules);
}

#[test]
fn retention_limits_are_cumulative_and_include_native_coefficient_bytes() {
    let (audit, mut solution) = solved_tadpole();
    let (_, mut another) = solved_tadpole();
    solution.rules.push(another.rules.remove(0));
    let one = audit
        .replay_sector_rule_circuits([true], None, &solution, &[0], Default::default())
        .unwrap();
    let limits = ReplayCircuitLimits {
        max_source_entries: one.circuits()[0].contributions().len(),
        ..Default::default()
    };
    assert!(
        audit
            .replay_sector_rule_circuits([true], None, &solution, &[1], limits)
            .is_ok()
    );
    assert!(matches!(
        audit.replay_sector_rule_circuits([true], None, &solution, &[0, 1], limits),
        Err(SourcePortAuditError::ResourceBudgetExhausted {
            resource: "retained replay sources"
        })
    ));
    let r = &one.circuits()[0];
    let coefficients = r
        .contributions()
        .iter()
        .map(|s| s.weight.raw())
        .chain(r.rhs().iter().map(|(_, c)| c.raw()))
        .chain(std::iter::once(r.raw_pivot().raw()));
    let polynomials = r
        .nonzero_conditions()
        .iter()
        .map(|p| p.raw())
        .chain(r.declared_exceptions().branches.iter().flatten());
    let bytes = coefficients
        .map(|c| coefficient_clone_owned_retained_byte_bound(c).unwrap())
        .sum::<usize>()
        + polynomials
            .map(|p| {
                polynomial_clone_owned_heap_byte_bound(p).unwrap()
                    + std::mem::size_of::<CoefficientPolynomial>()
            })
            .sum::<usize>();
    // Admission charges the original native allocations before retention.
    // Cloning RHS/condition vectors may compact capacity, so measure this
    // pre-copy bound rather than summing already-retained allocations.
    let (_, raw_bytes, _) = audit
        .with_replayed_sector_rule_iter(
            [true],
            None,
            &solution.order,
            &solution.rules,
            [0, 1].into_iter(),
            true,
            |_, rule, partition, checked| {
                let mut budget = RetentionBudget::new(Default::default());
                budget.charge(rule, &partition, &checked, audit.sources.conditions())?;
                Ok(budget.retained_bytes())
            },
        )
        .unwrap();
    eprintln!("retention bytes: post-admission={bytes}, pre-admission={raw_bytes:?}");
    assert!(raw_bytes.iter().sum::<usize>() > *raw_bytes.iter().max().unwrap());
    let limits = ReplayCircuitLimits {
        max_coefficient_clone_owned_bytes: *raw_bytes.iter().max().unwrap(),
        ..Default::default()
    };
    for ordinal in [0, 1] {
        audit
            .replay_sector_rule_circuits([true], None, &solution, &[ordinal], limits)
            .unwrap();
    }
    assert!(matches!(
        audit.replay_sector_rule_circuits([true], None, &solution, &[0, 1], limits),
        Err(SourcePortAuditError::ResourceBudgetExhausted {
            resource: "retained replay coefficient bytes"
        })
    ));
    for limits in [
        ReplayCircuitLimits {
            max_rules: 0,
            ..Default::default()
        },
        ReplayCircuitLimits {
            max_coefficient_clone_owned_bytes: 0,
            ..Default::default()
        },
        ReplayCircuitLimits {
            max_conditions: 0,
            ..Default::default()
        },
        ReplayCircuitLimits {
            max_coordinate_cells: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            audit.replay_sector_rule_circuits([true], None, &solution, &[0], limits),
            Err(SourcePortAuditError::ResourceBudgetExhausted { .. })
        ));
    }
}

#[test]
fn retained_circuit_keeps_existing_guard_context_and_order_rejections() {
    let (audit, mut solution) = solved_tadpole();
    assert!(
        audit
            .replay_sector_rule_circuits([true], Some([1]), &solution, &[0], Default::default())
            .is_err()
    );
    for ordinals in [vec![0, 0], vec![1], vec![1, 0]] {
        assert!(
            audit
                .replay_sector_rule_circuits([true], None, &solution, &ordinals, Default::default())
                .is_err()
        );
    }
    solution.rules[0].exceptions = Default::default();
    assert!(
        audit
            .replay_sector_rule_circuits([true], None, &solution, &[0], Default::default())
            .err()
            .unwrap()
            .to_string()
            .contains("exceptional guard branches")
    );
    let (_, mut solution) = solved_tadpole();
    solution.rules[0].candidate.rhs[0].coefficient =
        crate::algebra::CoefficientContext::new(["foreign"]).one();
    assert!(
        audit
            .replay_sector_rule_circuits([true], None, &solution, &[0], Default::default())
            .is_err()
    );
    let (_, mut solution) = solved_tadpole();
    solution.rules[0].candidate.sources[0].seed.shifts[0] += 1;
    assert!(
        audit
            .replay_sector_rule_circuits([true], None, &solution, &[0], Default::default())
            .is_err()
    );
    assert!(SourcePortAudit::<1>::try_new(&tadpole(), Arc::from([[true]])).is_err());
    assert!(SourcePortAudit::<1>::try_new(&tadpole(), Arc::from([[false], [false]])).is_err());
}

#[test]
fn affine_targets_and_exclusions_are_never_retained_as_rectangles() {
    let context = crate::algebra::CoefficientContext::new(["x", "y"]);
    let equality = context.coefficient_fixture("x-y").numerator;
    let affine = Case::<2>::generic()
        .intersect(&[equality], &[0, 1], &[true, true])
        .unwrap()
        .unwrap();
    let rule = crate::solver::SectorRule {
        candidate: crate::solver::RuleCandidate {
            target: affine.integral(),
            case: affine,
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: Default::default(),
        dispatch_policy: Default::default(),
    };
    let partition = geometry::ApplicationPartition {
        boxes: Vec::new(),
        affine_exclusions: Vec::new(),
    };
    assert!(matches!(
        require_coordinate(&rule, &partition, &[true, true]),
        Err(SourcePortAuditError::UnsupportedAffineOwnership {
            role: AffineOwnershipRole::Target,
            ..
        })
    ));
    let excluded =
        AffineApplicationDomain::from_case(rule.candidate.case.affine().unwrap(), &[true, true])
            .unwrap();
    let partition = geometry::ApplicationPartition {
        boxes: Vec::new(),
        affine_exclusions: vec![Arc::new(excluded)],
    };
    let case = Case::<2>::generic();
    let coordinate_rule = crate::solver::SectorRule {
        candidate: crate::solver::RuleCandidate {
            target: case.integral(),
            case,
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: Default::default(),
        dispatch_policy: Default::default(),
    };
    assert!(matches!(
        require_coordinate(&coordinate_rule, &partition, &[true, true]),
        Err(SourcePortAuditError::UnsupportedAffineOwnership {
            role: AffineOwnershipRole::Exceptional,
            ..
        })
    ));
}
