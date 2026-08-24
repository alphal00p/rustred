//! Focused provider-level checks for the V3 multi-persistent-source boundary.
//!
//! The sunset and tadpole are validation fixtures only. Production source
//! selection is entirely by authenticated sector/partial-assignment scope.

use std::sync::Arc;

use rustred::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits, AffineDenominator,
    CERTIFIED_FAMILY_RULE_PROVIDER_V1_SCHEMA, CERTIFIED_FAMILY_RULE_PROVIDER_V2_SCHEMA,
    CERTIFIED_FAMILY_RULE_PROVIDER_V3_SCHEMA, CertifiedConcreteRewriteProof,
    CertifiedFamilyRuleProvider, CertifiedFamilyRuleProviderError,
    CertifiedFamilyRuleProviderLimits, CoefficientContext, ConcreteIntegralKey,
    ConcreteRuleApplicationTrace, ConcreteRuleDecision, ConcreteRuleProvider,
    FamilySectorInventoryCertificate, FamilySectorInventoryCompiler, FamilySectorInventoryLimits,
    GeneratedCylindricalPersistentEliminationCertificate,
    GeneratedCylindricalPersistentEliminationLimits, GeneratedCylindricalRowSystemCertificate,
    GeneratedCylindricalRowSystemLimits, GeneratedCylindricalSectorRootStartCertificate,
    GeneratedCylindricalSectorRootStartLimits, GeneratedSymbolicRowSpanConfig, IntegralFamily,
    IntegralOrderingPolicy, InternalSymmetrySearchLimits, MasterPolicyProvider,
    ParametricIbpConfig, ParametricIbpGenerator, ParametricReductionEngine, PowerShiftPolicy,
    ReductionEngineLimits, SectorMask, SectorRestrictions,
    discover_bounded_vacuum_internal_symmetries,
};

const ORDERING: IntegralOrderingPolicy = IntegralOrderingPolicy::RustRedUnshiftedV1;

fn tadpole(name: &str) -> IntegralFamily {
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
    let minus_m2 = coefficients.parse("-m2").unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_m2.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_m2.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn inventory(family: &IntegralFamily) -> Arc<FamilySectorInventoryCertificate> {
    Arc::new(
        FamilySectorInventoryCompiler::compile(
            family,
            SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
            PowerShiftPolicy::FormalGeneric,
            ORDERING,
            FamilySectorInventoryLimits::default(),
        )
        .unwrap(),
    )
}

fn persistent_source(
    family: &IntegralFamily,
    context: &rustred::ParametricCoefficientContext,
    inventory: Arc<FamilySectorInventoryCertificate>,
    sector: SectorMask,
    through_depth: usize,
) -> Arc<GeneratedCylindricalPersistentEliminationCertificate> {
    let root = Arc::new(
        GeneratedCylindricalSectorRootStartCertificate::compile(
            family,
            context,
            inventory,
            sector,
            ParametricIbpConfig::default(),
            GeneratedSymbolicRowSpanConfig::default(),
            through_depth,
            GeneratedCylindricalSectorRootStartLimits::default(),
        )
        .unwrap(),
    );
    let rows = Arc::new(
        GeneratedCylindricalRowSystemCertificate::compile_from_sector_root(
            family,
            context,
            root,
            GeneratedCylindricalRowSystemLimits::default(),
        )
        .unwrap(),
    );
    Arc::new(
        GeneratedCylindricalPersistentEliminationCertificate::compile(
            family,
            context,
            rows,
            GeneratedCylindricalPersistentEliminationLimits::default(),
        )
        .unwrap(),
    )
}

fn persistent_source_scope_census(
    sources: &[Arc<GeneratedCylindricalPersistentEliminationCertificate>],
) -> (usize, usize) {
    sources.iter().fold(
        (0usize, 0usize),
        |(scope_entries, index_bytes), source| {
            let start = source.row_system().start();
            (
                scope_entries + start.sector().arity() + start.assignment().entries().len(),
                index_bytes
                    + std::mem::size_of::<
                        Arc<GeneratedCylindricalPersistentEliminationCertificate>,
                    >()
                    + start.sector().arity() * std::mem::size_of::<bool>()
                    + start.assignment().entries().len()
                        * std::mem::size_of::<(usize, i64)>(),
            )
        },
    )
}

#[test]
fn singular_compatibility_duplicate_resource_and_foreign_sources_are_explicit() {
    assert_eq!(
        CertifiedFamilyRuleProvider::SCHEMA,
        CERTIFIED_FAMILY_RULE_PROVIDER_V3_SCHEMA
    );
    assert_ne!(
        CERTIFIED_FAMILY_RULE_PROVIDER_V1_SCHEMA,
        CERTIFIED_FAMILY_RULE_PROVIDER_V3_SCHEMA
    );
    assert_ne!(
        CERTIFIED_FAMILY_RULE_PROVIDER_V2_SCHEMA,
        CERTIFIED_FAMILY_RULE_PROVIDER_V3_SCHEMA
    );

    let family = tadpole("multi-persistent-provider-tadpole");
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let persistent = persistent_source(
        &family,
        generated.context(),
        inventory(&family),
        SectorMask::try_new([true]).unwrap(),
        1,
    );

    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        ORDERING,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let no_source_provider = CertifiedFamilyRuleProvider::try_new(
        family.clone(),
        SectorRestrictions::unrestricted(1).unwrap(),
        [],
        adaptive,
        ORDERING,
        CertifiedFamilyRuleProviderLimits::default(),
    )
    .unwrap();
    assert!(no_source_provider.persistent_cylindrical_source().is_none());
    assert!(
        no_source_provider
            .persistent_cylindrical_sources()
            .is_empty()
    );

    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        ORDERING,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut singular_limits = CertifiedFamilyRuleProviderLimits::default();
    singular_limits.max_persistent_cylindrical_sources = 1;
    let (singular_scope_entries, singular_index_bytes) =
        persistent_source_scope_census(&[Arc::clone(&persistent)]);
    singular_limits.max_persistent_cylindrical_source_scope_entries = singular_scope_entries;
    singular_limits.max_persistent_cylindrical_source_index_bytes = singular_index_bytes;
    let provider = CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_source(
        family.clone(),
        SectorRestrictions::unrestricted(1).unwrap(),
        [],
        adaptive,
        Arc::clone(&persistent),
        ORDERING,
        singular_limits,
    )
    .unwrap();
    assert_eq!(provider.persistent_cylindrical_sources().len(), 1);
    assert!(Arc::ptr_eq(
        provider.persistent_cylindrical_source().unwrap(),
        &persistent
    ));

    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        ORDERING,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut one_below_scope = CertifiedFamilyRuleProviderLimits::default();
    one_below_scope.max_persistent_cylindrical_source_scope_entries = singular_scope_entries - 1;
    let scope_resource =
        match CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_sources(
            family.clone(),
            // The aggregate scope preflight must win over base-provider arity
            // replay, and it runs before any certificate authentication.
            SectorRestrictions::unrestricted(2).unwrap(),
            [],
            adaptive,
            [Arc::clone(&persistent)],
            ORDERING,
            one_below_scope,
        ) {
            Ok(_) => panic!("persistent scope entries above the configured cap were accepted"),
            Err(error) => error,
        };
    assert!(matches!(
        scope_resource,
        CertifiedFamilyRuleProviderError::ResourceLimit {
            resource: "persistent cylindrical source scope entries",
            requested,
            limit,
        } if requested == singular_scope_entries && limit + 1 == singular_scope_entries
    ));

    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        ORDERING,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut one_below_bytes = CertifiedFamilyRuleProviderLimits::default();
    one_below_bytes.max_persistent_cylindrical_source_scope_entries = singular_scope_entries;
    one_below_bytes.max_persistent_cylindrical_source_index_bytes = singular_index_bytes - 1;
    let byte_resource =
        match CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_sources(
            family.clone(),
            SectorRestrictions::unrestricted(2).unwrap(),
            [],
            adaptive,
            [Arc::clone(&persistent)],
            ORDERING,
            one_below_bytes,
        ) {
            Ok(_) => panic!("persistent index bytes above the configured cap were accepted"),
            Err(error) => error,
        };
    assert!(matches!(
        byte_resource,
        CertifiedFamilyRuleProviderError::ResourceLimit {
            resource: "persistent cylindrical source index bytes",
            requested,
            limit,
        } if requested == singular_index_bytes && limit + 1 == singular_index_bytes
    ));
    assert!(Arc::ptr_eq(
        &provider.persistent_cylindrical_sources()[0],
        &persistent
    ));

    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        ORDERING,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let duplicate = match CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_sources(
        family.clone(),
        // Wrong arity would fail base-provider construction. Duplicate scope
        // must be rejected first by the shallow source preflight.
        SectorRestrictions::unrestricted(2).unwrap(),
        [],
        adaptive,
        [Arc::clone(&persistent), Arc::clone(&persistent)],
        ORDERING,
        CertifiedFamilyRuleProviderLimits::default(),
    ) {
        Ok(_) => panic!("duplicate exact persistent scope was accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        duplicate,
        CertifiedFamilyRuleProviderError::DuplicatePersistentCylindricalSourceScope {
            sector,
            assignment,
        } if sector == SectorMask::try_new([true]).unwrap() && assignment.is_empty()
    ));

    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        ORDERING,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut limits = CertifiedFamilyRuleProviderLimits::default();
    limits.max_persistent_cylindrical_sources = 1;
    let resource = match CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_sources(
        family,
        // Likewise prove source count is checked before base-provider replay.
        SectorRestrictions::unrestricted(2).unwrap(),
        [],
        adaptive,
        [Arc::clone(&persistent), Arc::clone(&persistent)],
        ORDERING,
        limits,
    ) {
        Ok(_) => panic!("persistent source count above the configured cap was accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        resource,
        CertifiedFamilyRuleProviderError::ResourceLimit {
            resource: "persistent cylindrical sources",
            requested: 2,
            limit: 1,
        }
    ));

    let foreign_family = tadpole("multi-persistent-provider-foreign-tadpole");
    let foreign_generated = ParametricIbpGenerator::try_new(&foreign_family)
        .unwrap()
        .generate()
        .unwrap();
    let foreign_rows = foreign_generated.ibp_li().cloned().collect::<Vec<_>>();
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        foreign_generated.context(),
        &foreign_rows,
        ORDERING,
        AdaptiveRuleSearchLimits::default(),
    )
    .unwrap();
    let mut foreign_precedence_limits = CertifiedFamilyRuleProviderLimits::default();
    foreign_precedence_limits.max_persistent_cylindrical_source_scope_entries = 0;
    foreign_precedence_limits.max_persistent_cylindrical_source_index_bytes = 0;
    let foreign = match CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_sources(
        foreign_family,
        // Cheap foreign scope rejection precedes both the deliberately zero
        // aggregate index limits and wrong-arity base-provider construction.
        SectorRestrictions::unrestricted(2).unwrap(),
        [],
        adaptive,
        [persistent],
        ORDERING,
        foreign_precedence_limits,
    ) {
        Ok(_) => panic!("foreign persistent source was accepted"),
        Err(error) => error,
    };
    assert!(matches!(
        foreign,
        CertifiedFamilyRuleProviderError::ForeignPersistentCylindricalSource
    ));
}

#[test]
fn multiple_sector_sources_are_sorted_and_selected_with_exact_proof_provenance() {
    let family = equal_mass_sunset("multi-persistent-provider-sunset");
    let generated = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .generate()
        .unwrap();
    let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    let inventory = inventory(&family);
    let boundary_sector = SectorMask::try_new([false, true, true]).unwrap();
    let sunset_sector = SectorMask::try_new([true, true, true]).unwrap();
    let boundary = persistent_source(
        &family,
        generated.context(),
        Arc::clone(&inventory),
        boundary_sector.clone(),
        1,
    );
    let sunset = persistent_source(
        &family,
        generated.context(),
        inventory,
        sunset_sector.clone(),
        1,
    );
    let restrictions = SectorRestrictions::unrestricted(3).unwrap();
    let symmetry_report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(symmetry_report.completion().is_exhaustive_within_bounds());
    let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
    adaptive_limits.max_search_depth = 0;
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &rows,
        ORDERING,
        adaptive_limits,
    )
    .unwrap();
    let (aggregate_scope_entries, aggregate_index_bytes) =
        persistent_source_scope_census(&[Arc::clone(&sunset), Arc::clone(&boundary)]);
    let mut provider_limits = CertifiedFamilyRuleProviderLimits::default();
    // Exact aggregate boundaries must admit the complete two-source index.
    provider_limits.max_persistent_cylindrical_source_scope_entries = aggregate_scope_entries;
    provider_limits.max_persistent_cylindrical_source_index_bytes = aggregate_index_bytes;
    // Deliberately pass the lexicographically later scope first.
    let mut provider = CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_sources(
        family.clone(),
        restrictions,
        symmetry_report.symmetries().iter().cloned(),
        adaptive,
        [Arc::clone(&sunset), Arc::clone(&boundary)],
        ORDERING,
        provider_limits,
    )
    .unwrap();
    assert_eq!(provider.persistent_cylindrical_sources().len(), 2);
    assert!(provider.persistent_cylindrical_source().is_none());
    assert_eq!(
        provider.persistent_cylindrical_sources()[0]
            .row_system()
            .start()
            .sector(),
        &boundary_sector
    );
    assert_eq!(
        provider.persistent_cylindrical_sources()[1]
            .row_system()
            .start()
            .sector(),
        &sunset_sector
    );

    let boundary_dot = ConcreteIntegralKey::try_new([0, 1, 2]).unwrap();
    let boundary_rewrite = match provider.decision_for(&boundary_dot).unwrap() {
        ConcreteRuleDecision::CertifiedRewrite(rewrite) => rewrite,
        other => panic!("expected a boundary persistent-source rewrite, got {other:?}"),
    };
    let CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
        persistent_source,
        ..
    } = boundary_rewrite.proof()
    else {
        panic!("boundary rule did not retain persistent-source provenance")
    };
    assert!(Arc::ptr_eq(persistent_source, &boundary));
    assert!(!Arc::ptr_eq(persistent_source, &sunset));

    let top_dot = ConcreteIntegralKey::try_new([2, 1, 1]).unwrap();
    let top_first_step = match provider.decision_for(&top_dot).unwrap() {
        ConcreteRuleDecision::CertifiedRewrite(rewrite) => rewrite,
        other => panic!("expected a top-sector symmetry rewrite, got {other:?}"),
    };
    assert!(matches!(
        top_first_step.proof(),
        CertifiedConcreteRewriteProof::Symmetry { .. }
    ));

    let selected_masters = [
        ConcreteIntegralKey::try_new([0, 1, 1]).unwrap(),
        ConcreteIntegralKey::try_new([1, 1, 1]).unwrap(),
    ];
    let provider = MasterPolicyProvider::with_selected(provider, selected_masters).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        ORDERING,
        provider,
        ReductionEngineLimits::default(),
    );
    let closed = engine.reduce(&top_dot).unwrap();
    closed.require_complete().unwrap();
    assert_eq!(
        closed.terms(),
        &std::collections::BTreeMap::from([(
            ConcreteIntegralKey::try_new([1, 1, 1]).unwrap(),
            family.coefficient_context().parse("(d-3)/(3*m2)").unwrap(),
        )])
    );
    assert!(closed.application_traces().iter().any(|trace| matches!(
        trace,
        ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite)
            if matches!(rewrite.proof(), CertifiedConcreteRewriteProof::Symmetry { .. })
    )));
    let top_rewrite = closed
        .application_traces()
        .iter()
        .find_map(|trace| match trace {
            ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite)
                if matches!(
                    rewrite.proof(),
                    CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
                        persistent_source,
                        ..
                    } if Arc::ptr_eq(persistent_source, &sunset)
                ) =>
            {
                Some(rewrite.clone())
            }
            _ => None,
        })
        .expect("J(2,1,1) must select the exact top-sector persistent source");
    drop(engine);
    drop(boundary);
    drop(sunset);
    boundary_rewrite
        .replay(&family, generated.context(), ORDERING)
        .unwrap();
    top_rewrite
        .replay(&family, generated.context(), ORDERING)
        .unwrap();
}
