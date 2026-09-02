//! Focused offline diagnosis of the first AlphaLoop-itinerary guard wall.

use symbolica::prelude::Factorize;

use crate::foundry::artifact::{certify_alpha_to_rust_map, materialize_alpha_loop_lhs_anchors};
use crate::foundry::completion::LatticePoint;
use crate::foundry::completion::frame::exact::ExactCircuitGuardOrigin;
use crate::foundry::completion::source_discovery::leader_walk::{
    LeaderWalkLimits, RequestedDomain, RequestedDomainScopePartition, try_plan_requested_domains,
};
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::foundry::completion::source_discovery::{
    CampaignModularProbe, ExactExecutableOwnerObstruction, ProbeCampaignOutcome,
};

use super::super::{ProbeCampaignAdapter, ProbeCampaignLimits};

const PROBES: [(u64, i64); 6] = [
    (1_000_000_007, 31),
    (1_000_000_007, 37),
    (1_000_000_009, 31),
    (1_000_000_009, 37),
    (998_244_353, 31),
    (998_244_353, 37),
];

/// This test intentionally prints exact RustRed-owned circuit payload only.
/// AlphaLoop's RHS is never parsed or admitted as algebraic authority.
#[test]
#[ignore = "offline exact line-301 guard diagnosis"]
fn line301_guard_origin_and_final_relation_diagnostic() {
    let fixture = OracleDisabledK6Fixture::shared();
    let limits = ProbeCampaignLimits::default();
    let adapter = ProbeCampaignAdapter::try_new(
        fixture.generator(),
        fixture.completed(),
        fixture.zero_sources(),
        limits,
    )
    .unwrap();
    let anchor = materialize_alpha_loop_lhs_anchors()
        .into_iter()
        .find(|anchor| anchor.source.source_line == 301)
        .unwrap();
    let request = [RequestedDomain::new(
        LatticePoint::try_new(anchor.canonical_point.coordinates().iter().copied()).unwrap(),
        anchor.canonical_symbolic_axes.iter().copied(),
    )];
    let mut ledger = fixture.new_ledger_for_sector(&anchor.canonical_sector);
    let partition = ledger.try_clone_uncovered_partition().unwrap();
    let plan = try_plan_requested_domains(
        ledger.revision().get(),
        [RequestedDomainScopePartition::new(
            "line301-guard-diagnostic",
            &anchor.canonical_sector,
            &partition,
            &request,
        )],
        LeaderWalkLimits::default(),
    )
    .unwrap();
    let task = &plan.tasks()[0];
    let origin = task.base_probe_chart_origin().collect::<Vec<_>>();
    let probes = PROBES
        .into_iter()
        .enumerate()
        .map(|(probe_ordinal, (modulus, dimension))| {
            let coordinates = origin
                .iter()
                .enumerate()
                .map(|(position, &base)| {
                    let offset = ((probe_ordinal + position) % 3) as u64;
                    base.saturating_add(offset)
                })
                .collect::<Vec<_>>();
            CampaignModularProbe::try_new(
                modulus,
                [dimension],
                coordinates,
                limits.replay.scheduler.campaign,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let binding = adapter.try_bind_task(&plan, task, &ledger).unwrap();
    let report = adapter.try_run_task(binding, &mut ledger, probes).unwrap();
    let applied = match report.outcome() {
        ProbeCampaignOutcome::Duplicate(applied)
        | ProbeCampaignOutcome::ChangedWithoutGeometricShrink(applied)
        | ProbeCampaignOutcome::StrictGeometricShrink(applied)
        | ProbeCampaignOutcome::Closed { applied, .. } => applied,
        outcome => panic!("expected a published guard-free component, got {outcome:?}"),
    };
    assert_eq!(applied.obstructions().len(), 1);
    let obstruction = &applied.obstructions()[0];
    let ExactExecutableOwnerObstruction::ExceptionalGuardDomain { split, .. } =
        obstruction.obstruction()
    else {
        panic!("expected the line-301 exceptional-domain work item")
    };
    let circuit = obstruction.circuit();
    let cleared = obstruction.cleared();
    let context = fixture.generator().context();
    let alpha_map = certify_alpha_to_rust_map();
    let alpha_source_for_canonical_target = anchor
        .canonical_route
        .source_for_target()
        .iter()
        .map(|&raw_source| alpha_map.form_source_for_rust_target[raw_source])
        .collect::<Vec<_>>();
    assert_eq!(alpha_source_for_canonical_target, [1, 0, 2, 4, 3, 5]);
    eprintln!(
        "LINE301 canonical_route={:?} alpha_source_for_canonical_target={alpha_source_for_canonical_target:?}",
        anchor.canonical_route.source_for_target(),
    );
    eprintln!(
        "LINE301 production_split=guard:{} position:{} value:{} circuit target={:?} residuals={} sources={} pivots={} raw_guards={}",
        split.guard_ordinal(),
        split.position(),
        split.value(),
        circuit.target_shift().values(),
        circuit.residual_terms().len(),
        circuit.source_combination().len(),
        circuit.pivot_guards().len(),
        circuit.nonzero_guards().len(),
    );
    assert!(cleared.is_bound_to(circuit));
    let raw_guard = &circuit.nonzero_guards()[2];
    assert_eq!(raw_guard.polynomial().to_expression().to_string(), "-1+n2");
    assert_eq!(raw_guard.origins().len(), 1);
    let ExactCircuitGuardOrigin::ReducerPivotNumerator {
        frame_row_ordinal,
        source_instance,
        physical_pivot_column,
    } = &raw_guard.origins()[0]
    else {
        panic!("raw guard ordinal 2 must have the diagnosed pivot origin")
    };
    assert_eq!((*frame_row_ordinal, *physical_pivot_column), (17, 39));
    assert_eq!(source_instance.provenance().source_ordinal(), 5);
    assert_eq!(
        source_instance.provenance().source_row().stable_string(),
        "ordinary-ibp:1:2"
    );
    assert_eq!(
        source_instance.provenance().offset().values(),
        &[0, 0, -1, 0, 0, 1]
    );
    assert_eq!(
        circuit
            .source_combination()
            .iter()
            .map(|source| source.frame_row_ordinal())
            .collect::<Vec<_>>(),
        [1, 4, 22, 23, 38]
    );
    eprintln!(
        "LINE301 raw_guard[2]={} chronology={:?}",
        raw_guard.polynomial().to_expression(),
        raw_guard.origins(),
    );

    let target_at_wall = context
        .specialize_fixed_polynomial(cleared.target_coefficient(), &[(2, 1)], Default::default())
        .unwrap();
    assert!(!target_at_wall.is_zero());
    assert_eq!(cleared.semantic_guards().len(), 1);
    let genuine_at_alpha_n5_eq_1 = context
        .specialize_fixed_polynomial(
            cleared.semantic_guards()[0].polynomial(),
            &[(3, 1)],
            Default::default(),
        )
        .unwrap();
    assert!(genuine_at_alpha_n5_eq_1.is_zero());
    let semantic_at_spurious_wall = context
        .specialize_fixed_polynomial(
            cleared.semantic_guards()[0].polynomial(),
            &[(2, 1)],
            Default::default(),
        )
        .unwrap();
    assert!(!semantic_at_spurious_wall.is_zero());
    assert_eq!(
        (split.guard_ordinal(), split.position(), split.value()),
        (0, 3, 1)
    );
    assert_eq!(split.admitted_domain().bounds()[3].lower(), 2);
    assert_eq!(
        split.admitted_domain().bounds()[3].upper(),
        obstruction.epoch().fixed_stratum().domain().bounds()[3].upper()
    );
    assert_eq!(split.exceptional_domain().bounds()[3].lower(), 1);
    assert_eq!(split.exceptional_domain().bounds()[3].upper(), 1);
    assert!(split.deferred_guard_free_domain().is_none());
    eprintln!(
        "LINE301 cleared target={} target_at_position2_eq_1={} telemetry={:?} sources={} physical_terms={} exact_operations={} gcd_term_pairs={} retained_terms={}",
        cleared.target_coefficient().to_expression(),
        target_at_wall.to_expression(),
        cleared.guard_telemetry(),
        cleared.source_cofactors().len(),
        cleared.physical_terms().len(),
        cleared.exact_operations(),
        cleared.gcd_term_pairs(),
        cleared.retained_polynomial_terms(),
    );
    let guard = &cleared.semantic_guards()[0];
    let expected_guard = context
        .mul(
            &context.index(5).unwrap(),
            &context
                .sub(&context.index(3).unwrap(), &context.one())
                .unwrap(),
        )
        .unwrap();
    let expected_guard = context
        .numerator_condition_with_limits(&expected_guard, Default::default())
        .unwrap();
    assert_eq!(guard.polynomial(), &expected_guard);
    let native_factors = guard.polynomial().raw().factor();
    let factor_report = native_factors
        .iter()
        .map(|(factor, multiplicity)| {
            let factor = context
                .admit_native_polynomial_result_with_limits(factor.clone(), Default::default())
                .unwrap();
            let coefficient_system = context
                .base_coefficient_system(&factor, Default::default(), Default::default())
                .unwrap();
            let zero_set = context
                .univariate_integer_zero_set(&coefficient_system, Default::default())
                .unwrap();
            (factor.to_expression().to_string(), *multiplicity, zero_set)
        })
        .collect::<Vec<_>>();
    let bounds = obstruction.epoch().fixed_stratum().domain().bounds();
    let coefficient_system = context
        .base_coefficient_system(guard.polynomial(), Default::default(), Default::default())
        .unwrap();
    let zero_locus_misses_domain = context
        .integer_zero_locus_misses_domain(
            &coefficient_system,
            Default::default(),
            |position, root| {
                root.to_i64()
                    .is_some_and(|value| bounds[position].contains(value))
            },
        )
        .unwrap();
    assert!(!zero_locus_misses_domain);
    eprintln!(
        "LINE301 Symbolica factors={factor_report:?} application_bounds={bounds:?} zero_locus_misses_domain={zero_locus_misses_domain}",
    );
    eprintln!(
        "LINE301 semantic_guard[0]={} at_position2_eq_1={} at_position3_eq_1={} origins={:?}",
        guard.polynomial().to_expression(),
        semantic_at_spurious_wall.to_expression(),
        genuine_at_alpha_n5_eq_1.to_expression(),
        guard.origins(),
    );
}
