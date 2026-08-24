//! Black-box validation of the generic family-wide generated-rule pipeline.
//!
//! The one-loop tadpole and connected two-loop sunset are fixtures only.  No
//! topology name, loop-count dispatch, recurrence, master list, or oracle is
//! passed to the production compiler.

use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, CutConstraint, GeneratedFamilyRuleSystemCompiler,
    GeneratedFamilyRuleSystemConfig, GeneratedFamilyRuleSystemError,
    GeneratedFamilyRuleSystemLimits, GeneratedFamilySectorFailure, GeneratedFamilySectorResource,
    GeneratedFamilySectorStatus, GeneratedSectorDiscoveryError, GeneratedSymbolicRowSpanError,
    IntegralFamily, IntegralOrderingPolicy, ParametricIbpGenerator, PowerShiftPolicy, SectorMask,
    SectorPattern, SectorRestrictions,
};

fn mask(bits: &str) -> SectorMask {
    SectorMask::try_from_bit_string(bits).unwrap()
}

fn massive_tadpole(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let mass = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(mass.clone(), vec![one.clone(), zero.clone(), zero.clone()]),
            AffineDenominator::new(mass.clone(), vec![zero.clone(), zero.clone(), one.clone()]),
            AffineDenominator::new(mass, vec![one.clone(), coefficients.integer(-2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn compile(
    family: &IntegralFamily,
    limits: GeneratedFamilyRuleSystemLimits,
) -> Result<rustred::GeneratedFamilyRuleSystemCertificate, GeneratedFamilyRuleSystemError> {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    GeneratedFamilyRuleSystemCompiler::compile(
        family,
        &context,
        SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedFamilyRuleSystemConfig::default(),
        limits,
    )
}

#[test]
fn one_loop_compiles_inventory_discovery_and_exceptional_queue_without_master_inference() {
    let family = massive_tadpole("generated-family-one-loop");
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let certificate = GeneratedFamilyRuleSystemCompiler::compile(
        &family,
        &context,
        SectorRestrictions::unrestricted(1).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedFamilyRuleSystemConfig::default(),
        GeneratedFamilyRuleSystemLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.sectors().len(), 2);
    assert_eq!(certificate.solve_order(), [mask("1")]);
    assert!(matches!(
        certificate.status(&mask("0")),
        Some(GeneratedFamilySectorStatus::ProvedZero(_))
    ));
    let Some(GeneratedFamilySectorStatus::Unresolved {
        no_zero_certificate,
        solve_ordinal,
        discovery,
        live_leaf_queue,
    }) = certificate.status(&mask("1"))
    else {
        panic!("the active tadpole sector should complete the generated pipeline")
    };
    assert_eq!(*solve_ordinal, 0);
    assert_eq!(no_zero_certificate.raw_sector(), &mask("1"));
    assert_eq!(discovery.sector(), &mask("1"));
    assert_eq!(live_leaf_queue.sector(), &mask("1"));
    assert_eq!(certificate.stats().proved_zero(), 1);
    assert_eq!(certificate.stats().unresolved(), 1);
    assert_eq!(certificate.stats().failed(), 0);
    assert_eq!(certificate.stats().resource_limited(), 0);
    assert!(certificate.stats().generated_candidate_attempts() > 0);
    assert_eq!(
        certificate.stats().shared_row_span_compilation_attempts(),
        1
    );
    assert_eq!(certificate.stats().shared_row_span_certificates(), 1);
    assert_eq!(certificate.stats().shared_row_span_sector_reuses(), 1);
    assert_eq!(
        certificate.stats().shared_row_span_candidate_reuses(),
        certificate.stats().generated_candidate_attempts()
    );
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn discovery_resource_and_algorithm_failures_are_retained_not_promoted_to_masters() {
    let family = massive_tadpole("generated-family-interruptions");
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();

    let mut resource_limits = GeneratedFamilyRuleSystemLimits::default();
    resource_limits.discovery.max_candidate_layers = 0;
    let resource = compile(&family, resource_limits).unwrap();
    assert!(matches!(
        resource.status(&mask("1")),
        Some(GeneratedFamilySectorStatus::ResourceLimited {
            no_zero_certificate: Some(_),
            solve_ordinal: Some(0),
            completed_discovery: None,
            resource: GeneratedFamilySectorResource::Discovery(_),
        })
    ));
    assert_eq!(resource.stats().resource_limited(), 1);
    assert_eq!(resource.stats().unresolved(), 0);
    resource.replay(&family, &context).unwrap();

    let mut failure_limits = GeneratedFamilyRuleSystemLimits::default();
    failure_limits
        .discovery
        .adaptive
        .rule
        .arithmetic
        .max_source_terms -= 1;
    let failure = compile(&family, failure_limits).unwrap();
    assert!(matches!(
        failure.status(&mask("1")),
        Some(GeneratedFamilySectorStatus::Failed {
            no_zero_certificate: Some(_),
            solve_ordinal: Some(0),
            completed_discovery: None,
            failure: GeneratedFamilySectorFailure::Discovery(_),
        })
    ));
    assert_eq!(failure.stats().failed(), 1);
    assert_eq!(failure.stats().unresolved(), 0);
    failure.replay(&family, &context).unwrap();
}

#[test]
fn inventory_resource_statuses_do_not_enter_the_generated_solve_order() {
    let family = massive_tadpole("generated-family-inventory-resource");
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut limits = GeneratedFamilyRuleSystemLimits::default();
    limits.inventory.zero_sectors.max_rank_rows = 0;
    let certificate = compile(&family, limits).unwrap();
    assert!(certificate.solve_order().is_empty());
    assert_eq!(certificate.stats().discovery_attempts(), 0);
    assert_eq!(
        certificate.stats().shared_row_span_compilation_attempts(),
        0
    );
    assert_eq!(certificate.stats().shared_row_span_certificates(), 0);
    assert_eq!(certificate.stats().shared_row_span_sector_reuses(), 0);
    assert_eq!(certificate.stats().shared_row_span_candidate_reuses(), 0);
    assert!(certificate.sectors().iter().any(|entry| matches!(
        entry.status(),
        GeneratedFamilySectorStatus::ResourceLimited {
            no_zero_certificate: None,
            solve_ordinal: None,
            completed_discovery: None,
            resource: GeneratedFamilySectorResource::Inventory(_),
        }
    )));
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn one_shared_row_span_interruption_stops_before_sector_discovery_and_replays() {
    let family = massive_tadpole("generated-family-shared-row-span-resource");
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut limits = GeneratedFamilyRuleSystemLimits::default();
    limits
        .discovery
        .coverage
        .generated_when_bad
        .row_span
        .limits
        .max_canonical_rows = 0;
    let certificate = compile(&family, limits).unwrap();

    assert!(certificate.row_span_arc().is_none());
    assert!(matches!(
        certificate.row_span_interruption(),
        Some(GeneratedSymbolicRowSpanError::ResourceLimit {
            resource: "generated row-span canonical rows",
            requested: 1,
            limit: 0,
        })
    ));
    assert!(matches!(
        certificate.status(&mask("1")),
        Some(GeneratedFamilySectorStatus::ResourceLimited {
            no_zero_certificate: Some(_),
            solve_ordinal: Some(0),
            completed_discovery: None,
            resource: GeneratedFamilySectorResource::Discovery(
                GeneratedSectorDiscoveryError::RowSpan(
                    GeneratedSymbolicRowSpanError::ResourceLimit {
                        resource: "generated row-span canonical rows",
                        requested: 1,
                        limit: 0,
                    }
                )
            ),
        })
    ));
    assert_eq!(
        certificate.stats().shared_row_span_compilation_attempts(),
        1
    );
    assert_eq!(certificate.stats().shared_row_span_certificates(), 0);
    assert_eq!(certificate.stats().shared_row_span_sector_reuses(), 0);
    assert_eq!(certificate.stats().shared_row_span_candidate_reuses(), 0);
    assert_eq!(certificate.stats().discovery_attempts(), 0);
    assert_eq!(certificate.stats().completed_discoveries(), 0);
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn connected_two_loop_fixture_completes_all_unresolved_sector_queues_and_replays() {
    let family = equal_mass_sunset("generated-family-connected-sunset");
    let mut limits = GeneratedFamilyRuleSystemLimits::default();
    limits.discovery.adaptive.max_search_depth = 0;
    limits.live_leaf_queue.translation_radius = 0;
    limits.live_leaf_queue.max_translation_points = 1;
    let certificate = compile(&family, limits).unwrap();

    assert_eq!(certificate.sectors().len(), 8);
    assert_eq!(
        certificate
            .solve_order()
            .iter()
            .map(SectorMask::to_bit_string)
            .collect::<Vec<_>>(),
        ["011", "101", "110", "111"]
    );
    for (ordinal, sector) in certificate.solve_order().iter().enumerate() {
        let status = certificate.status(sector).unwrap();
        assert_eq!(status.solve_ordinal(), Some(ordinal));
        assert!(status.no_zero_certificate().is_some());
        assert!(
            status.is_unresolved() || status.is_resource_limited() || status.is_failed(),
            "an analyzed full-rank sector must retain its exact generated-stage outcome"
        );
    }
    assert_eq!(certificate.stats().discovery_attempts(), 4);
    assert_eq!(certificate.stats().completed_discoveries(), 4);
    assert_eq!(certificate.stats().live_leaf_queue_attempts(), 4);
    assert_eq!(certificate.stats().completed_live_leaf_queues(), 4);
    assert_eq!(certificate.stats().unresolved(), 4);
    assert_eq!(certificate.stats().resource_limited(), 0);
    assert_eq!(certificate.stats().failed(), 0);
    assert!(certificate.stats().generated_candidate_attempts() > 0);
    assert_eq!(certificate.stats().proved_zero(), 4);
    assert_eq!(
        certificate.stats().shared_row_span_compilation_attempts(),
        1
    );
    assert_eq!(certificate.stats().shared_row_span_certificates(), 1);
    assert_eq!(certificate.stats().shared_row_span_sector_reuses(), 4);
    assert_eq!(
        certificate.stats().shared_row_span_candidate_reuses(),
        certificate.stats().generated_candidate_attempts()
    );
    let shared_row_span = certificate
        .row_span_arc()
        .expect("generated-stage work must retain one family row span");
    for sector in certificate.solve_order() {
        let Some(GeneratedFamilySectorStatus::Unresolved {
            discovery,
            live_leaf_queue,
            ..
        }) = certificate.status(sector)
        else {
            panic!("the connected fixture completes every generated-sector pipeline")
        };
        assert!(Arc::ptr_eq(shared_row_span, discovery.row_span_arc()));
        assert!(Arc::ptr_eq(
            shared_row_span,
            discovery.coverage().row_span_arc()
        ));
        for attempt in discovery.coverage().candidate_attempts() {
            assert!(Arc::ptr_eq(
                shared_row_span,
                attempt.compilation().source_authentication().row_span_arc()
            ));
        }
        assert!(Arc::ptr_eq(
            shared_row_span,
            live_leaf_queue.discovery().row_span_arc()
        ));
    }
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn restrictions_govern_inventory_scheduling_but_are_not_misattributed_to_generated_rules() {
    let family = equal_mass_sunset("generated-family-restricted-sunset");
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let restrictions = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(3, [0]).unwrap(),
        SectorPattern::try_from_string("*0*").unwrap(),
    )
    .unwrap();
    let mut limits = GeneratedFamilyRuleSystemLimits::default();
    limits.discovery.adaptive.max_search_depth = 0;
    limits.live_leaf_queue.translation_radius = 0;
    limits.live_leaf_queue.max_translation_points = 1;
    let certificate = GeneratedFamilyRuleSystemCompiler::compile(
        &family,
        &context,
        restrictions.clone(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedFamilyRuleSystemConfig::default(),
        limits,
    )
    .unwrap();

    assert_eq!(certificate.inventory_restrictions(), &restrictions);
    assert_eq!(
        certificate.inventory_power_shift_policy(),
        PowerShiftPolicy::FormalGeneric
    );
    assert_eq!(certificate.stats().excluded(), 6);
    assert_eq!(certificate.stats().proved_zero(), 1);
    assert_eq!(certificate.solve_order(), [mask("101")]);
    assert_eq!(certificate.stats().discovery_attempts(), 1);
    assert!(matches!(
        certificate.status(&mask("001")),
        Some(GeneratedFamilySectorStatus::Excluded(_))
    ));
    assert!(
        certificate
            .status(&mask("101"))
            .unwrap()
            .no_zero_certificate()
            .is_some()
    );
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn family_wide_transcript_limits_fail_before_partial_retention() {
    let family = massive_tadpole("generated-family-outer-limit");
    let mut limits = GeneratedFamilyRuleSystemLimits::default();
    limits.max_sector_transcripts = 1;
    assert!(matches!(
        compile(&family, limits),
        Err(GeneratedFamilyRuleSystemError::ResourceLimit {
            resource: "family generated-rule sector transcripts",
            requested: 2,
            limit: 1,
        })
    ));
}
