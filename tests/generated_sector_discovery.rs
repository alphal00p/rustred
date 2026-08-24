//! Black-box validation of automatic generated-sector discovery.
//!
//! No recurrence, seed equation, elimination-pivot selection, master count,
//! topology name, or loop-specific rule is an input.  Multi-anchor tests
//! supply only generic same-sector search origins.

use std::sync::Arc;

use rustred::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchError, AffineDenominator,
    CertifiedRewriteLimits, CertifiedZeroSectorRuleProvider, CoefficientContext,
    ConcreteIntegralKey, GENERATED_SECTOR_DISCOVERY_V1_SCHEMA,
    GENERATED_SECTOR_DISCOVERY_V3_SCHEMA, GENERATED_SECTOR_DISCOVERY_V4_SCHEMA,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryError,
    GeneratedSectorDiscoveryLimits, GeneratedSectorSearchAnchorRequest,
    GeneratedWhenBadCompilation, GeneratedWhenBadCompiler, IntegralFamily, IntegralOrderingPolicy,
    MasterPolicyProvider, ParametricCoefficientContext, ParametricEliminationError,
    ParametricIbpGenerator, ParametricReductionEngine, ParametricRuleApplication,
    ParametricSectorLeafDisposition, ParametricSectorRuleProvider,
    ParametricSectorRuleProviderLimits, PowerShiftPolicy, ReductionEngineLimits, SectorMask,
    WhenBadLeafDisposition,
};

fn family_named(name: &str) -> IntegralFamily {
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
            AffineDenominator::new(mass, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn default_context(family: &IntegralFamily) -> ParametricCoefficientContext {
    ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone()
}

fn key(power: i64) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new([power]).unwrap()
}

fn compile(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    active: bool,
    limits: GeneratedSectorDiscoveryLimits,
) -> Result<rustred::GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryError> {
    GeneratedSectorDiscoveryCompiler::compile(
        family,
        context,
        SectorMask::try_new([active]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        limits,
    )
}

#[test]
fn automatic_active_search_covers_every_n_from_two_and_leaves_n_one_uncovered() {
    let family = family_named("generated-sector-discovery-active");
    let context = default_context(&family);
    let certificate = compile(
        &family,
        &context,
        true,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();

    assert_eq!(certificate.schema(), GENERATED_SECTOR_DISCOVERY_V1_SCHEMA);
    assert_eq!(certificate.family_fingerprint(), family.fingerprint());
    assert_eq!(certificate.context_fingerprint(), context.fingerprint());
    assert_eq!(certificate.corner(), [1]);
    assert_eq!(certificate.candidate_counts_by_layer().len(), 3);
    assert_eq!(certificate.stats().canonical_rows(), 1);
    assert_eq!(certificate.stats().canonical_terms(), 2);
    assert_eq!(certificate.stats().candidate_layers(), 3);
    assert_eq!(
        certificate
            .candidate_counts_by_layer()
            .iter()
            .sum::<usize>(),
        certificate.stats().candidate_attempts()
    );
    assert!(certificate.stats().candidate_attempts() > 0);
    assert!(certificate.stats().certified_candidates() > 0);
    // Structural composition retains duplicate predicate branches from every
    // authenticated candidate; only the concrete point classification is the
    // semantic coverage claim.
    assert!(certificate.stats().uncovered_leaves() > 0);
    certificate.replay(&family, &context).unwrap();

    let coverage = certificate.coverage();
    assert!(matches!(
        coverage
            .classification_for_indices(&context, &[1])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::Uncovered
    ));
    for power in [2, 3, 17, i64::MAX] {
        assert!(matches!(
            coverage
                .classification_for_indices(&context, &[power])
                .unwrap()
                .unwrap()
                .disposition(),
            ParametricSectorLeafDisposition::DescendingRule { .. }
        ));
    }
    assert!(
        coverage
            .classification_for_indices(&context, &[0])
            .unwrap()
            .is_none()
    );
}

#[test]
fn arbitrary_same_sector_anchors_are_canonical_replayable_and_share_one_coverage() {
    let family = family_named("generated-sector-discovery-multi-anchor");
    let context = default_context(&family);
    let mut limits = GeneratedSectorDiscoveryLimits::default();
    limits.adaptive.max_search_depth = 0;
    let corner = compile(&family, &context, true, limits).unwrap();
    let shared = corner.row_span_arc().clone();

    // Deliberately supply the anchors in reverse order.  V3 authenticates a
    // canonical search order and composes every retained candidate into one
    // exact global sector-case coverage.
    let certificate = GeneratedSectorDiscoveryCompiler::compile_with_search_anchors_and_row_span(
        &family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        [key(2), key(1)],
        shared.clone(),
        limits,
    )
    .unwrap();
    assert_eq!(certificate.schema(), GENERATED_SECTOR_DISCOVERY_V3_SCHEMA);
    assert_eq!(
        certificate
            .search_anchors()
            .iter()
            .map(|entry| entry.anchor().powers().to_vec())
            .collect::<Vec<_>>(),
        [vec![1], vec![2]]
    );
    assert!(
        certificate
            .search_anchors()
            .iter()
            .all(|entry| entry.candidate_counts_by_layer().len() == 1)
    );
    assert_eq!(certificate.stats().candidate_layers(), 2);
    assert_eq!(
        certificate
            .candidate_counts_by_layer()
            .iter()
            .sum::<usize>(),
        certificate.stats().candidate_attempts()
    );
    assert!(Arc::ptr_eq(certificate.row_span_arc(), &shared));
    assert!(Arc::ptr_eq(certificate.coverage().row_span_arc(), &shared));
    certificate.replay(&family, &context).unwrap();

    for power in [2, 3, 19] {
        assert!(matches!(
            certificate
                .coverage()
                .classification_for_indices(&context, &[power])
                .unwrap()
                .unwrap()
                .disposition(),
            ParametricSectorLeafDisposition::DescendingRule { .. }
        ));
    }
}

#[test]
fn arbitrary_anchor_validation_and_retained_layer_limits_fail_closed() {
    let family = family_named("generated-sector-discovery-anchor-limits");
    let context = default_context(&family);
    let mut limits = GeneratedSectorDiscoveryLimits::default();
    limits.adaptive.max_search_depth = 0;
    let corner = compile(&family, &context, true, limits).unwrap();
    let shared = corner.row_span_arc().clone();
    let sector = SectorMask::try_new([true]).unwrap();

    let compile_anchors = |anchors: Vec<ConcreteIntegralKey>, limits| {
        GeneratedSectorDiscoveryCompiler::compile_with_search_anchors_and_row_span(
            &family,
            &context,
            sector.clone(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            anchors,
            shared.clone(),
            limits,
        )
    };

    assert!(matches!(
        compile_anchors(Vec::new(), limits),
        Err(GeneratedSectorDiscoveryError::EmptySearchAnchors)
    ));
    assert!(matches!(
        compile_anchors(vec![key(1), key(1)], limits),
        Err(GeneratedSectorDiscoveryError::DuplicateSearchAnchor { anchor })
            if anchor == key(1)
    ));
    assert!(matches!(
        compile_anchors(vec![key(0)], limits),
        Err(GeneratedSectorDiscoveryError::SearchAnchorOutsideSector { anchor })
            if anchor == key(0)
    ));
    assert!(matches!(
        compile_anchors(vec![ConcreteIntegralKey::try_new([1, 1]).unwrap()], limits,),
        Err(GeneratedSectorDiscoveryError::WrongSearchAnchorArity {
            expected: 1,
            actual: 2,
        })
    ));

    let mut anchor_count = limits;
    anchor_count.max_search_anchors = 1;
    assert!(matches!(
        compile_anchors(vec![key(1), key(2)], anchor_count),
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource: "generated-sector search anchors",
            requested: 2,
            limit: 1,
        })
    ));

    let mut components = limits;
    components.max_search_anchor_components = 1;
    assert!(matches!(
        compile_anchors(vec![key(1), key(2)], components),
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource: "generated-sector search anchor components",
            requested: 2,
            limit: 1,
        })
    ));

    let mut retained = limits;
    retained.max_total_anchor_layer_entries = 1;
    assert!(matches!(
        compile_anchors(vec![key(1), key(2)], retained),
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource: "generated-sector anchor layer entries",
            requested: 2,
            limit: 1,
        })
    ));

    let mut summed_mixed_depths = limits;
    summed_mixed_depths.adaptive.max_search_depth = 1;
    summed_mixed_depths.max_total_anchor_layer_entries = 2;
    assert!(matches!(
        GeneratedSectorDiscoveryCompiler::compile_with_search_anchor_requests_and_row_span(
            &family,
            &context,
            sector.clone(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            [
                GeneratedSectorSearchAnchorRequest::new(key(1), 0),
                GeneratedSectorSearchAnchorRequest::new(key(2), 1),
            ],
            shared.clone(),
            summed_mixed_depths,
        ),
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource: "generated-sector anchor layer entries",
            requested: 3,
            limit: 2,
        })
    ));

    let mut global_max_is_only_a_hard_bound = limits;
    global_max_is_only_a_hard_bound.adaptive.max_search_depth = 100;
    global_max_is_only_a_hard_bound.max_candidate_layers = 1;
    global_max_is_only_a_hard_bound.max_retained_layer_entries = 1;
    global_max_is_only_a_hard_bound.max_total_anchor_layer_entries = 1;
    let shallow =
        GeneratedSectorDiscoveryCompiler::compile_with_search_anchor_requests_and_row_span(
            &family,
            &context,
            sector.clone(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            [GeneratedSectorSearchAnchorRequest::new(key(1), 0)],
            shared.clone(),
            global_max_is_only_a_hard_bound,
        )
        .unwrap();
    assert_eq!(shallow.candidate_counts_by_layer().len(), 1);
    assert_eq!(shallow.stats().candidate_layers(), 1);

    assert!(matches!(
        GeneratedSectorDiscoveryCompiler::compile_with_search_anchor_requests_and_row_span(
            &family,
            &context,
            sector.clone(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            [GeneratedSectorSearchAnchorRequest::new(key(1), 101)],
            shared.clone(),
            global_max_is_only_a_hard_bound,
        ),
        Err(
            GeneratedSectorDiscoveryError::SearchAnchorDepthExceedsMaximum {
                requested: 101,
                maximum: 100,
                ..
            }
        )
    ));

    let mut v4_ordering_limits = limits;
    v4_ordering_limits.adaptive.max_search_depth = 1;
    let v4 = GeneratedSectorDiscoveryCompiler::compile_with_search_anchor_requests_and_row_span(
        &family,
        &context,
        sector.clone(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        [
            GeneratedSectorSearchAnchorRequest::new(key(2), 1),
            GeneratedSectorSearchAnchorRequest::new(key(1), 0),
        ],
        shared.clone(),
        v4_ordering_limits,
    )
    .unwrap();
    assert_eq!(v4.schema(), GENERATED_SECTOR_DISCOVERY_V4_SCHEMA);
    assert_eq!(
        v4.search_anchors()
            .iter()
            .map(|entry| (
                entry.anchor().powers().to_vec(),
                entry.maximum_local_depth()
            ))
            .collect::<Vec<_>>(),
        [(vec![1], 0), (vec![2], 1)]
    );
    v4.replay(&family, &context).unwrap();

    assert!(matches!(
        GeneratedSectorDiscoveryCompiler::compile_with_search_anchor_requests_and_row_span(
            &family,
            &context,
            sector,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            [
                GeneratedSectorSearchAnchorRequest::new(key(1), 0),
                GeneratedSectorSearchAnchorRequest::new(key(1), 1),
            ],
            shared,
            v4_ordering_limits,
        ),
        Err(GeneratedSectorDiscoveryError::DuplicateSearchAnchor { anchor }) if anchor == key(1)
    ));
}

#[test]
fn multi_anchor_candidate_budget_is_aggregate_before_the_next_derivation_payload() {
    let family = family_named("generated-sector-discovery-anchor-candidate-cap");
    let context = default_context(&family);
    let mut limits = GeneratedSectorDiscoveryLimits::default();
    limits.adaptive.max_search_depth = 0;
    let corner = compile(&family, &context, true, limits).unwrap();
    let shared = corner.row_span_arc().clone();
    let first_count = corner.stats().candidate_attempts();
    assert!(first_count > 0);

    // The first anchor exactly consumes the aggregate allowance.  The second
    // adaptive search is recreated with a zero remaining pivot allowance, so
    // it is rejected before constructing its rule-derivation payload.
    limits.coverage.max_candidates = first_count;
    assert!(matches!(
        GeneratedSectorDiscoveryCompiler::compile_with_search_anchors_and_row_span(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            [key(1), key(2)],
            shared,
            limits,
        ),
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource: "generated-sector candidate attempts",
            requested,
            limit,
        }) if requested > first_count && limit == first_count
    ));
}

#[test]
fn sunset_residual_search_derives_and_replays_exact_j_minus_one_one_one_rule() {
    let family = equal_mass_sunset("generated-sector-discovery-sunset-residual-anchor");
    let context = default_context(&family);
    let sector = SectorMask::try_new([false, true, true]).unwrap();
    let mut base_limits = GeneratedSectorDiscoveryLimits::default();
    base_limits.adaptive.max_search_depth = 0;
    let base = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        sector.clone(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        base_limits,
    )
    .unwrap();
    let shared = base.row_span_arc().clone();

    let mut search_limits = base_limits.adaptive;
    search_limits.max_search_depth = 1;
    let residual = ConcreteIntegralKey::try_new([-1, 1, 1]).unwrap();
    let mut search = AdaptiveParametricRuleProvider::try_new(
        &context,
        base.row_span().rows(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        search_limits,
    )
    .unwrap();
    let mut covering = None;
    let mut ordered_candidate_ordinal = 0usize;
    for layer in search.candidate_layers_for_quotient(&residual).unwrap() {
        for candidate in layer {
            let compilation = GeneratedWhenBadCompiler::compile_with_row_span(
                &family,
                &context,
                &candidate,
                shared.clone(),
                base_limits.coverage.generated_when_bad,
            )
            .unwrap();
            let GeneratedWhenBadCompilation::Certified(certificate) = compilation else {
                ordered_candidate_ordinal += 1;
                continue;
            };
            let covers_residual = certificate
                .admissibility()
                .classification_for_indices(&context, residual.powers())
                .unwrap()
                .is_some_and(|leaf| {
                    matches!(
                        leaf.disposition(),
                        WhenBadLeafDisposition::CoveredByCandidate
                    )
                });
            if covers_residual {
                covering = Some((ordered_candidate_ordinal, certificate));
                break;
            }
            ordered_candidate_ordinal += 1;
        }
        if covering.is_some() {
            break;
        }
    }
    let (ordered_candidate_ordinal, certificate) =
        covering.expect("generated residual search did not find a certified descending pivot");
    assert_eq!(ordered_candidate_ordinal, 14);
    // The deterministic cumulative stencil may select a pivot whose own
    // elimination anchor differs from the requested residual point; coverage
    // and concrete application, not anchor equality, certify usefulness.
    assert!(Arc::ptr_eq(
        certificate.source_authentication().row_span_arc(),
        &shared
    ));
    certificate
        .replay_with_row_span(&family, &context, shared)
        .unwrap();

    let reduction = match certificate
        .admissibility()
        .candidate()
        .apply(&context, residual.powers())
        .unwrap()
    {
        ParametricRuleApplication::Applicable(reduction) => reduction,
        application => panic!("certified residual pivot was not applicable: {application:?}"),
    };
    assert_eq!(reduction.source(), &residual);
    assert_eq!(reduction.rhs().len(), 4);
    let coefficients = family.coefficient_context();
    assert_eq!(
        reduction
            .rhs()
            .get(&ConcreteIntegralKey::try_new([0, 0, 1]).unwrap()),
        Some(&coefficients.parse("1/(d-1)").unwrap())
    );
    assert_eq!(
        reduction
            .rhs()
            .get(&ConcreteIntegralKey::try_new([0, 0, 2]).unwrap()),
        Some(&coefficients.parse("2*m2/(d-1)").unwrap())
    );
    assert_eq!(
        reduction
            .rhs()
            .get(&ConcreteIntegralKey::try_new([0, 1, 0]).unwrap()),
        Some(&coefficients.parse("-1/(d-1)").unwrap())
    );
    assert_eq!(
        reduction
            .rhs()
            .get(&ConcreteIntegralKey::try_new([0, 1, 1]).unwrap()),
        Some(&coefficients.parse("m2").unwrap())
    );
    assert!(reduction.replay_application(&family, &context).unwrap());
}

#[test]
#[ignore = "full 27-candidate symbolic global composition and replay currently exceeds two minutes"]
fn sunset_v4_global_composition_covers_j_minus_one_one_one() {
    let family = equal_mass_sunset("generated-sector-discovery-sunset-v4-global");
    let context = default_context(&family);
    let sector = SectorMask::try_new([false, true, true]).unwrap();
    let mut base_limits = GeneratedSectorDiscoveryLimits::default();
    base_limits.adaptive.max_search_depth = 0;
    let base = GeneratedSectorDiscoveryCompiler::compile(
        &family,
        &context,
        sector.clone(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        base_limits,
    )
    .unwrap();
    let shared = base.row_span_arc().clone();
    let mut limits = base_limits;
    limits.adaptive.max_search_depth = 1;
    limits.coverage.max_candidates = 27;
    let certificate =
        GeneratedSectorDiscoveryCompiler::compile_with_search_anchor_requests_and_row_span(
            &family,
            &context,
            sector,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            [
                GeneratedSectorSearchAnchorRequest::new(
                    ConcreteIntegralKey::try_new([0, 1, 1]).unwrap(),
                    0,
                ),
                GeneratedSectorSearchAnchorRequest::new(
                    ConcreteIntegralKey::try_new([-1, 1, 1]).unwrap(),
                    1,
                ),
            ],
            shared.clone(),
            limits,
        )
        .unwrap();
    assert_eq!(certificate.schema(), GENERATED_SECTOR_DISCOVERY_V4_SCHEMA);
    assert_eq!(certificate.search_anchors().len(), 2);
    assert_eq!(
        certificate
            .search_anchors()
            .iter()
            .map(|entry| (
                entry.anchor().powers().to_vec(),
                entry.maximum_local_depth()
            ))
            .collect::<Vec<_>>(),
        [(vec![0, 1, 1], 0), (vec![-1, 1, 1], 1)]
    );
    let classification = certificate
        .coverage()
        .classification_for_indices(&context, &[-1, 1, 1])
        .unwrap()
        .unwrap();
    let candidate_ordinal = match classification.disposition() {
        ParametricSectorLeafDisposition::DescendingRule { candidate_ordinal } => *candidate_ordinal,
        disposition => panic!("residual anchor stayed unresolved: {disposition:?}"),
    };
    assert!(certificate.coverage().candidate_attempts()[candidate_ordinal].is_certified());
    assert!(Arc::ptr_eq(certificate.row_span_arc(), &shared));
    assert!(Arc::ptr_eq(certificate.coverage().row_span_arc(), &shared));
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn automatic_inactive_search_is_explicitly_unsupported_not_a_master_claim() {
    let family = family_named("generated-sector-discovery-inactive");
    let context = default_context(&family);
    // The canonical depth-zero pivot moves outward forever on n <= 0 and is
    // therefore Unsupported.  A deeper automatic stencil is allowed to find
    // the distinct sound rule that descends toward n=0; this test isolates
    // the required fail-closed unsupported candidate without supplying it.
    let mut limits = GeneratedSectorDiscoveryLimits::default();
    limits.adaptive.max_search_depth = 0;
    let certificate = compile(&family, &context, false, limits).unwrap();
    certificate.replay(&family, &context).unwrap();
    assert_eq!(certificate.corner(), [0]);
    assert!(certificate.stats().unsupported_candidates() > 0);
    assert!(certificate.stats().unsupported_leaves() > 0);

    for power in [0, -1, -17, i64::MIN] {
        assert!(matches!(
            certificate
                .coverage()
                .classification_for_indices(&context, &[power])
                .unwrap()
                .unwrap()
                .disposition(),
            ParametricSectorLeafDisposition::Unsupported {
                candidate_ordinals,
            } if !candidate_ordinals.is_empty()
        ));
    }
    assert!(
        certificate
            .coverage()
            .classification_for_indices(&context, &[1])
            .unwrap()
            .is_none()
    );
}

#[test]
fn deeper_automatic_inactive_stencil_finds_a_descending_zero_chain_not_a_master() {
    let family = family_named("generated-sector-discovery-inactive-deeper");
    let context = default_context(&family);
    let certificate = compile(
        &family,
        &context,
        false,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    certificate.replay(&family, &context).unwrap();

    // Unlike the unsupported outward depth-zero pivot above, a deeper
    // cumulative stencil contains a distinct rule descending toward n=0.
    // At n=0 its raised term vanishes and the same generated identity proves
    // the integral zero on the generic coefficient locus.
    for power in [0, -1, -17] {
        assert!(matches!(
            certificate
                .coverage()
                .classification_for_indices(&context, &[power])
                .unwrap()
                .unwrap()
                .disposition(),
            ParametricSectorLeafDisposition::DescendingRule { .. }
        ));
    }

    let provider = ParametricSectorRuleProvider::try_new(
        &family,
        &context,
        [certificate.coverage().clone()],
        ParametricSectorRuleProviderLimits::default(),
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );
    let result = engine.reduce(&key(-3)).unwrap();
    result.require_complete().unwrap();
    assert!(result.terms().is_empty());
    assert!(result.terminal_statuses().is_empty());
    assert!(result.selected_masters().is_empty());
    assert!(result.certified_masters().is_empty());
}

#[test]
fn caller_owned_context_is_bound_exactly_and_replays_only_in_that_namespace() {
    let family = family_named("generated-sector-discovery-context");
    let context = ParametricCoefficientContext::try_new(
        family.coefficient_context(),
        "caller-owned-generated-sector-scope",
        1,
    )
    .unwrap();
    let certificate = compile(
        &family,
        &context,
        true,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    certificate.replay(&family, &context).unwrap();
    assert_eq!(certificate.context_fingerprint(), context.fingerprint());
    assert!(matches!(
        certificate
            .coverage()
            .classification_for_indices(&context, &[2])
            .unwrap()
            .unwrap()
            .disposition(),
        ParametricSectorLeafDisposition::DescendingRule { .. }
    ));

    let canonical = default_context(&family);
    assert!(matches!(
        certificate.replay(&family, &canonical),
        Err(GeneratedSectorDiscoveryError::WrongContext)
    ));
    assert!(matches!(
        certificate
            .coverage()
            .classification_for_indices(&canonical, &[2]),
        Err(rustred::ParametricSectorCoverageError::WrongContext)
    ));
}

#[test]
fn layer_depth_candidate_and_canonical_budgets_fail_closed_then_retry_succeeds() {
    let family = family_named("generated-sector-discovery-budgets");
    let context = default_context(&family);

    let mut depth_overflow = GeneratedSectorDiscoveryLimits::default();
    depth_overflow.adaptive.max_search_depth = usize::MAX;
    assert!(matches!(
        compile(&family, &context, true, depth_overflow),
        Err(GeneratedSectorDiscoveryError::ResourceCountOverflow {
            resource: "generated-sector search depth layers",
        })
    ));

    let mut depth = GeneratedSectorDiscoveryLimits::default();
    depth.max_candidate_layers = 2;
    assert!(matches!(
        compile(&family, &context, true, depth),
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource: "generated-sector search depth layers",
            requested: 3,
            limit: 2,
        })
    ));

    let mut retained_layers = GeneratedSectorDiscoveryLimits::default();
    retained_layers.max_retained_layer_entries = 2;
    assert!(matches!(
        compile(&family, &context, true, retained_layers),
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource: "generated-sector retained layer entries",
            requested: 3,
            limit: 2,
        })
    ));

    let mut candidates = GeneratedSectorDiscoveryLimits::default();
    candidates.coverage.max_candidates = 0;
    // Depth zero consumes one offset. Without the outer-cap clamp, the helper
    // would allocate a derivation or continue into this depth-one offset trap.
    candidates.adaptive.max_enumerated_offsets_per_integral = 1;
    candidates.adaptive.rule.max_source_rows_for_replay = 0;
    assert!(matches!(
        compile(&family, &context, true, candidates),
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource: "generated-sector candidate attempts",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let mut source_rows = GeneratedSectorDiscoveryLimits::default();
    source_rows.adaptive.elimination.max_source_rows = 0;
    assert!(matches!(
        compile(&family, &context, true, source_rows),
        Err(GeneratedSectorDiscoveryError::Adaptive(
            AdaptiveRuleSearchError::Elimination(ParametricEliminationError::ResourceLimit {
                resource: "source rows",
                requested: 1,
                limit: 0,
            })
        ))
    ));

    let mut rows = GeneratedSectorDiscoveryLimits::default();
    rows.coverage.generated_when_bad.max_canonical_rows = 0;
    assert!(matches!(
        compile(&family, &context, true, rows),
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource: "generated-sector canonical rows",
            requested: 1,
            limit: 0,
        })
    ));

    let mut terms = GeneratedSectorDiscoveryLimits::default();
    terms.coverage.generated_when_bad.max_canonical_terms = 1;
    assert!(matches!(
        compile(&family, &context, true, terms),
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource: "generated-sector canonical terms",
            requested: 2,
            limit: 1,
        })
    ));

    let mut adaptive = GeneratedSectorDiscoveryLimits::default();
    adaptive.adaptive.max_enumerated_offsets_per_integral = 0;
    assert!(matches!(
        compile(&family, &context, true, adaptive),
        Err(GeneratedSectorDiscoveryError::Adaptive(
            AdaptiveRuleSearchError::ResourceLimit { limit: 0, .. }
        ))
    ));

    // Compilation owns no mutable search object: every failed attempt above
    // is transactional with respect to the caller's family and context.
    let certificate = compile(
        &family,
        &context,
        true,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    certificate.replay(&family, &context).unwrap();
}

#[test]
fn every_cross_phase_arithmetic_mismatch_fails_before_the_search_resource_trap() {
    let family = family_named("generated-sector-discovery-coherence");
    let context = default_context(&family);

    let mut ibp_elimination = GeneratedSectorDiscoveryLimits::default();
    ibp_elimination
        .adaptive
        .elimination
        .arithmetic
        .max_source_terms -= 1;
    ibp_elimination.adaptive.max_enumerated_offsets_per_integral = 0;
    assert!(matches!(
        compile(&family, &context, true, ibp_elimination),
        Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "IBP authentication and stencil-elimination arithmetic policies differ",
        })
    ));

    let mut elimination_rule = GeneratedSectorDiscoveryLimits::default();
    elimination_rule.adaptive.rule.arithmetic.max_source_terms -= 1;
    elimination_rule
        .adaptive
        .max_enumerated_offsets_per_integral = 0;
    assert!(matches!(
        compile(&family, &context, true, elimination_rule),
        Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "stencil-elimination and rule-candidate arithmetic policies differ",
        })
    ));

    let mut rule_when_bad = GeneratedSectorDiscoveryLimits::default();
    rule_when_bad
        .coverage
        .generated_when_bad
        .when_bad
        .arithmetic
        .max_source_terms -= 1;
    rule_when_bad.adaptive.max_enumerated_offsets_per_integral = 0;
    assert!(matches!(
        compile(&family, &context, true, rule_when_bad),
        Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "rule-candidate and WhenBad arithmetic policies differ",
        })
    ));

    let mut local_cases = GeneratedSectorDiscoveryLimits::default();
    local_cases
        .coverage
        .generated_when_bad
        .when_bad
        .sector_cases
        .exact_algebra
        .max_polynomial_terms -= 1;
    local_cases.adaptive.max_enumerated_offsets_per_integral = 0;
    assert!(matches!(
        compile(&family, &context, true, local_cases),
        Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "WhenBad arithmetic and local sector-case exact-algebra policies differ",
        })
    ));

    let mut global_cases = GeneratedSectorDiscoveryLimits::default();
    global_cases
        .coverage
        .sector_cases
        .exact_algebra
        .max_polynomial_terms -= 1;
    global_cases.adaptive.max_enumerated_offsets_per_integral = 0;
    assert!(matches!(
        compile(&family, &context, true, global_cases),
        Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "WhenBad arithmetic and global sector-case exact-algebra policies differ",
        })
    ));
}

#[test]
fn automatic_coverage_composes_with_zero_outer_and_explicit_i_one_master_for_i_four() {
    let family = family_named("generated-sector-discovery-provider-chain");
    let context = default_context(&family);
    let discovery = compile(
        &family,
        &context,
        true,
        GeneratedSectorDiscoveryLimits::default(),
    )
    .unwrap();
    let sector_provider = ParametricSectorRuleProvider::try_new(
        &family,
        &context,
        [discovery.coverage().clone()],
        ParametricSectorRuleProviderLimits::default(),
    )
    .unwrap();
    let master_provider = MasterPolicyProvider::with_selected(sector_provider, [key(1)]).unwrap();
    let provider = CertifiedZeroSectorRuleProvider::try_unrestricted(
        &family,
        PowerShiftPolicy::FormalGeneric,
        master_provider,
        CertifiedRewriteLimits::default(),
    )
    .unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        provider,
        ReductionEngineLimits::default(),
    );

    let result = engine.reduce(&key(4)).unwrap();
    result.require_complete().unwrap();
    assert_eq!(result.selected_masters().len(), 1);
    assert!(result.selected_masters().contains(&key(1)));
    assert_eq!(result.terms().len(), 1);
    assert_eq!(
        result.terms().get(&key(1)).unwrap(),
        &family
            .coefficient_context()
            .parse("(d-6)*(d-4)*(d-2)/(48*m2^3)")
            .unwrap()
    );

    let zero = engine.reduce(&key(0)).unwrap();
    zero.require_complete().unwrap();
    assert!(zero.terms().is_empty());
    assert!(zero.terminal_statuses().is_empty());

    let stats = engine.provider().inner().inner().stats();
    assert_eq!(stats.queries(), 3);
    assert_eq!(stats.reductions(), 3);
    assert_eq!(stats.uncovered(), 0);
    assert_eq!(stats.unsupported(), 0);
}
