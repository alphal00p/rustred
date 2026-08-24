//! Current-path one-loop scalar acceptance against the frozen Vakint oracle.
//!
//! The massive tadpole is a validation family only.  Production receives no
//! recurrence, expected coefficient, preferred pivot, or loop-count dispatch:
//! the test constructs the generic family, generates its parametric IBP, builds
//! an anchor-free cylindrical sector-root source, persists its exact
//! elimination transcript, and requires the numeric zero/symmetry quotient to
//! supply every nontrivial rewrite.

use std::collections::BTreeMap;
use std::sync::Arc;

use rustred::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchLimits, AdaptiveRuleSearchStats,
    AffineDenominator, CertifiedConcreteRewrite, CertifiedConcreteRewriteProof,
    CertifiedFamilyRuleProvider, CertifiedFamilyRuleProviderLimits, CertifiedZeroReduction,
    Coefficient, CoefficientContext, ConcreteIntegralKey, ConcreteRuleApplicationTrace,
    FamilySectorInventoryCompiler, FamilySectorInventoryLimits,
    GeneratedCylindricalPersistentEliminationCertificate,
    GeneratedCylindricalPersistentEliminationLimits, GeneratedCylindricalRowSystemCertificate,
    GeneratedCylindricalRowSystemLimits, GeneratedCylindricalSectorRootStartCertificate,
    GeneratedCylindricalSectorRootStartLimits, GeneratedSymbolicRowSpanConfig, IntegralFamily,
    IntegralOrderingPolicy, InternalSymmetrySearchLimits, MasterPolicyProvider,
    ParametricIbpConfig, ParametricIbpGenerator, ParametricReductionEngine, PowerShiftPolicy,
    ReductionEngineLimits, SectorMask, SectorRestrictions,
    discover_bounded_vacuum_internal_symmetries,
};

const CYLINDRICAL_THROUGH_DEPTH: usize = 1;

fn massive_tadpole() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "generated-cylindrical-one-loop-scalar-oracle",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        // Vakint convention: k^2 = D1 + mUV^2.
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn key(power: i64) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new([power]).unwrap()
}

fn expected(
    context: &CoefficientContext,
    coefficient: &str,
) -> BTreeMap<ConcreteIntegralKey, Coefficient> {
    BTreeMap::from([(key(1), context.parse(coefficient).unwrap())])
}

#[test]
fn persistent_cylindrical_tadpole_reduces_powers_two_through_six_and_proves_zeros() {
    let family = massive_tadpole();
    let ordering = IntegralOrderingPolicy::RustRedUnshiftedV1;
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let restrictions = SectorRestrictions::unrestricted(family.denominator_count()).unwrap();
    let inventory = Arc::new(
        FamilySectorInventoryCompiler::compile(
            &family,
            restrictions.clone(),
            PowerShiftPolicy::FormalGeneric,
            ordering,
            FamilySectorInventoryLimits::default(),
        )
        .unwrap(),
    );
    let root = Arc::new(
        GeneratedCylindricalSectorRootStartCertificate::compile(
            &family,
            &context,
            inventory,
            SectorMask::try_new([true]).unwrap(),
            ParametricIbpConfig::default(),
            GeneratedSymbolicRowSpanConfig::default(),
            CYLINDRICAL_THROUGH_DEPTH,
            GeneratedCylindricalSectorRootStartLimits::default(),
        )
        .unwrap(),
    );
    assert!(root.assignment().is_empty());
    assert_eq!(root.row_span().rows().len(), 1, "L(L+E)=1 native IBP");

    let rows = Arc::new(
        GeneratedCylindricalRowSystemCertificate::compile_from_sector_root(
            &family,
            &context,
            root,
            GeneratedCylindricalRowSystemLimits::default(),
        )
        .unwrap(),
    );
    let persistent = Arc::new(
        GeneratedCylindricalPersistentEliminationCertificate::compile(
            &family,
            &context,
            rows,
            GeneratedCylindricalPersistentEliminationLimits::default(),
        )
        .unwrap(),
    );
    persistent.replay(&family, &context).unwrap();
    assert_eq!(persistent.stats().elimination_builds(), 1);
    assert!(persistent.stats().pivot_rows() > 0);

    let generated = ParametricIbpGenerator::try_with_context(
        &family,
        context.clone(),
        ParametricIbpConfig::default(),
    )
    .unwrap()
    .generate()
    .unwrap();
    let canonical_rows = generated.ibp_li().cloned().collect::<Vec<_>>();
    assert_eq!(canonical_rows.len(), 1);

    let symmetry_report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(symmetry_report.completion().is_exhaustive_within_bounds());

    // The API requires a nonempty ordinary generated-row fallback. Depth zero
    // still includes its central scout point, so make every adaptive
    // work/output surface hostile as well. Entering either fallback path must
    // fail before it can produce an unobserved rewrite.
    let mut adaptive_limits = AdaptiveRuleSearchLimits::default();
    adaptive_limits.max_search_depth = 0;
    adaptive_limits.max_enumerated_offsets_per_integral = 0;
    adaptive_limits.max_offset_enumeration_steps_per_layer = 0;
    adaptive_limits.max_offset_components_per_integral = 0;
    adaptive_limits.max_scout_points_per_integral = 0;
    adaptive_limits.max_pivot_candidates_per_integral = 0;
    adaptive_limits.max_cached_decisions = 0;
    adaptive_limits.elimination.max_source_rows = 0;
    adaptive_limits.elimination.max_columns = 0;
    adaptive_limits.elimination.max_pivots = 0;
    adaptive_limits.rule.max_rhs_terms = 0;
    adaptive_limits.rule.max_source_rows_for_replay = 0;
    let adaptive = AdaptiveParametricRuleProvider::try_new(
        generated.context(),
        &canonical_rows,
        ordering,
        adaptive_limits,
    )
    .unwrap();
    let provider = CertifiedFamilyRuleProvider::try_new_with_persistent_cylindrical_source(
        family.clone(),
        restrictions,
        symmetry_report.symmetries().iter().cloned(),
        adaptive,
        Arc::clone(&persistent),
        ordering,
        CertifiedFamilyRuleProviderLimits::default(),
    )
    .unwrap();
    assert_eq!(provider.adaptive().limits(), adaptive_limits);
    assert!(Arc::ptr_eq(
        provider.persistent_cylindrical_source().unwrap(),
        &persistent,
    ));
    let provider = MasterPolicyProvider::with_selected(provider, [key(1)]).unwrap();
    let mut engine = ParametricReductionEngine::new(
        family.fingerprint(),
        family.coefficient_context(),
        ordering,
        provider,
        ReductionEngineLimits::default(),
    );

    let expected_coefficients = [
        (2, "(d-2)/(2*m2)"),
        (3, "(d-4)*(d-2)/(8*m2^2)"),
        (4, "(d-6)*(d-4)*(d-2)/(48*m2^3)"),
        (5, "(d-8)*(d-6)*(d-4)*(d-2)/(384*m2^4)"),
        (6, "(d-10)*(d-8)*(d-6)*(d-4)*(d-2)/(3840*m2^5)"),
    ];
    let mut retained_rewrites = Vec::<CertifiedConcreteRewrite>::new();
    for (power, coefficient) in expected_coefficients {
        let result = engine.reduce(&key(power)).unwrap();
        result.require_complete().unwrap();
        assert_eq!(
            result.terms(),
            &expected(family.coefficient_context(), coefficient),
            "persistent cylindrical tadpole reduction mismatch at power {power}",
        );
        assert_eq!(
            result.selected_masters(),
            &std::collections::BTreeSet::from([key(1)])
        );
        assert!(result.required_nonzero().iter().any(|condition| {
            condition
                .polynomial()
                .to_expression()
                .to_string()
                .contains("m2")
        }));
        assert!(!result.application_traces().is_empty());
        for trace in result.application_traces() {
            let ConcreteRuleApplicationTrace::CertifiedRewrite(rewrite) = trace else {
                panic!("power {power} used a non-persistent application path: {trace:?}")
            };
            assert!(matches!(
                rewrite.proof(),
                CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
                    persistent_source,
                    ..
                } if Arc::ptr_eq(persistent_source, &persistent)
            ));
            retained_rewrites.push(rewrite.clone());
        }
    }

    let mut retained_zeros = Vec::<CertifiedZeroReduction>::new();
    for power in [0, -1, -2] {
        let result = engine.reduce(&key(power)).unwrap();
        result.require_complete().unwrap();
        assert!(result.terms().is_empty());
        assert!(result.terminal_statuses().is_empty());
        let [ConcreteRuleApplicationTrace::ProvedZero(proof)] = result.application_traces() else {
            panic!("nonpositive power {power} did not retain exactly one zero proof")
        };
        retained_zeros.push(proof.clone());
    }

    assert_eq!(
        engine.provider().inner().adaptive().stats(),
        AdaptiveRuleSearchStats::default()
    );
    drop(engine);
    // The retained rewrites must own everything needed for replay, including
    // the exact persistent source allocation.
    drop(persistent);
    assert!(!retained_rewrites.is_empty());
    for rewrite in retained_rewrites {
        rewrite.replay(&family, &context, ordering).unwrap();
    }
    for zero in retained_zeros {
        zero.replay(&family).unwrap();
    }
}
